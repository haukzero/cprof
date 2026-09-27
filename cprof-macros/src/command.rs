use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Expr, ExprArray, Ident, ItemFn, Member, Path, PathArguments, ReturnType, Token,
    parse::{Parse, ParseStream},
    parse_quote,
    visit_mut::{self, VisitMut},
};

mod kw {
    syn::custom_keyword!(no_retry);
}

struct Retry {
    operation: Expr,
    args: ExprArray,
    flags: Vec<(Expr, Expr)>,
}

pub(crate) struct Command {
    retry: Option<Retry>,
}

impl Parse for Command {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(kw::no_retry) {
            input.parse::<kw::no_retry>()?;
            if !input.is_empty() {
                return Err(input.error("no_retry cannot have other options"));
            }
            return Ok(Self { retry: None });
        }
        let mut operation = None;
        let mut args = None;
        let mut flags = None;
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            match key.to_string().as_str() {
                "retry" if operation.is_none() => operation = Some(input.parse::<Expr>()?),
                "args" if args.is_none() => args = Some(input.parse::<ExprArray>()?),
                "flags" if flags.is_none() => flags = Some(input.parse::<ExprArray>()?),
                _ => {
                    return Err(syn::Error::new_spanned(
                        key,
                        "unknown or duplicate command option",
                    ));
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        let operation = operation.ok_or_else(|| {
            input.error("expected no_retry or retry = function or receiver.method")
        })?;
        if !matches!(&operation, Expr::Path(_) | Expr::Field(_)) {
            return Err(syn::Error::new_spanned(
                operation,
                "expected function or receiver.method",
            ));
        }
        let args = args.ok_or_else(|| input.error("retry requires args = [...]"))?;
        if args.elems.is_empty() {
            return Err(syn::Error::new_spanned(
                args,
                "replay arguments cannot be empty",
            ));
        }
        let flags = flags
            .into_iter()
            .flat_map(|flags| flags.elems)
            .map(|flag| match flag {
                Expr::Tuple(tuple) if tuple.elems.len() == 2 => {
                    let mut parts = tuple.elems.into_iter();
                    Ok((parts.next().unwrap(), parts.next().unwrap()))
                }
                flag => Err(syn::Error::new_spanned(flag, "expected (condition, flag)")),
            })
            .collect::<syn::Result<Vec<_>>>()?;
        Ok(Self {
            retry: Some(Retry {
                operation,
                args,
                flags,
            }),
        })
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    left.leading_colon.is_some() == right.leading_colon.is_some()
        && left.segments.len() == right.segments.len()
        && left
            .segments
            .iter()
            .zip(&right.segments)
            .all(|(left, right)| {
                left.ident == right.ident
                    && matches!(left.arguments, PathArguments::None)
                    && matches!(right.arguments, PathArguments::None)
            })
}

impl Retry {
    fn matches(&self, expression: &Expr) -> bool {
        match (&self.operation, expression) {
            (Expr::Path(expected), Expr::Call(call)) => {
                matches!(call.func.as_ref(), Expr::Path(actual) if same_path(&expected.path, &actual.path))
            }
            (Expr::Field(expected), Expr::MethodCall(call)) => {
                matches!(&expected.member, Member::Named(method) if method == &call.method)
                    && matches!((expected.base.as_ref(), call.receiver.as_ref()),
                        (Expr::Path(expected), Expr::Path(actual)) if same_path(&expected.path, &actual.path))
            }
            _ => false,
        }
    }

    fn wrap(&self, operation: &Expr) -> Expr {
        let args = self
            .args
            .elems
            .iter()
            .map(|arg| quote!(::std::ffi::OsString::from(#arg)));
        let variable = Ident::new("__cprof_replay_args", Span::mixed_site());
        let mutable = (!self.flags.is_empty()).then(|| quote!(mut));
        let flags = self.flags.iter().map(|(condition, flag)| {
            quote! {
                if #condition {
                    #variable.push(::std::ffi::OsString::from(#flag));
                }
            }
        });
        parse_quote! {
            crate::elevate::run(
                || {
                    let #mutable #variable = vec![#(#args),*];
                    #(#flags)*
                    Ok(#variable)
                },
                || #operation,
            )
        }
    }
}

struct RetryVisitor<'a> {
    retry: &'a Retry,
    matches: usize,
}

impl VisitMut for RetryVisitor<'_> {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        if self.retry.matches(expression) {
            self.matches += 1;
            *expression = self.retry.wrap(expression);
        } else {
            visit_mut::visit_expr_mut(self, expression);
        }
    }

    // Nested functions define a separate command/operation boundary.
    fn visit_item_fn_mut(&mut self, _: &mut ItemFn) {}
}

pub(crate) fn expand(command: Command, mut function: ItemFn) -> syn::Result<TokenStream> {
    if function.sig.ident != "run" || function.sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "command requires a synchronous run function",
        ));
    }
    if let Some(retry) = command.retry {
        let mut visitor = RetryVisitor {
            retry: &retry,
            matches: 0,
        };
        visitor.visit_block_mut(&mut function.block);
        if visitor.matches != 1 {
            return Err(syn::Error::new_spanned(
                retry.operation,
                "retry must match exactly one operation in run",
            ));
        }
    }
    let original_output = match &function.sig.output {
        ReturnType::Type(_, ty) => ty.clone(),
        ReturnType::Default => {
            return Err(syn::Error::new_spanned(
                &function.sig,
                "command run must return Result<()> for dispatch",
            ));
        }
    };
    let original = function.block;
    function.sig.output = parse_quote!(-> crate::error::Result<crate::commands::CommandCompletion>);
    function.block = Box::new(parse_quote!({
        let result: #original_output = (|| #original)();
        result.map(|()| crate::commands::CommandCompletion::new())
    }));
    Ok(quote! {
        #function
    })
}

#[cfg(test)]
mod tests {
    use super::{Command, expand};

    #[test]
    fn requires_an_explicit_retry_policy() {
        assert!(
            syn::parse_str::<Command>("no_retry")
                .unwrap()
                .retry
                .is_none()
        );
        for options in [
            "",
            "no_retry, args = [\"test\"]",
            "retry = update",
            "retry = update, args = []",
        ] {
            assert!(syn::parse_str::<Command>(options).is_err(), "{options}");
        }
    }

    #[test]
    fn rejects_missing_or_ambiguous_retry_operations() {
        for function in [
            "fn run() { other(); }",
            "fn run() { update(); update(); }",
            "fn run() { fn nested() { update(); } }",
        ] {
            let options = syn::parse_str("retry = update, args = [\"test\"]").unwrap();
            let function = syn::parse_str(function).unwrap();
            let error = expand(options, function).unwrap_err();
            assert_eq!(
                error.to_string(),
                "retry must match exactly one operation in run"
            );
        }
    }

    #[test]
    fn run_returns_a_dispatch_completion() {
        let options = syn::parse_str::<Command>("no_retry").unwrap();
        let function = syn::parse_quote! {
            pub fn run() -> Result<()> { Ok(()) }
        };
        let expanded = expand(options, function).unwrap().to_string();
        assert!(expanded.contains("Result < crate :: commands :: CommandCompletion >"));
        assert!(expanded.contains("let result : Result < () >"));
        assert!(!expanded.contains("COMMAND_MARKER"));
    }
}
