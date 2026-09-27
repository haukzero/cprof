use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Expr, ExprCall, ItemFn,
    visit::{self, Visit},
};

#[derive(Default)]
struct CommandCalls {
    checks: Vec<TokenStream>,
}

impl<'ast> Visit<'ast> for CommandCalls {
    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        if let Expr::Path(path) = call.func.as_ref() {
            let segments = &path.path.segments;
            if segments.len() == 3
                && (segments[0].ident == "root" || segments[0].ident == "target")
                && segments[2].ident == "run"
            {
                let group = &segments[0];
                let command = &segments[1];
                self.checks.push(quote! {
                    const _: () = #group::#command::COMMAND_MARKER;
                });
            }
        }
        visit::visit_expr_call(self, call);
    }
}

pub(crate) fn expand(args: TokenStream, function: ItemFn) -> syn::Result<TokenStream> {
    if !args.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            "command_dispatch takes no arguments",
        ));
    }
    let mut calls = CommandCalls::default();
    calls.visit_item_fn(&function);
    let checks = calls.checks;
    Ok(quote! {
        #function
        #(#checks)*
    })
}

#[cfg(test)]
mod tests {
    use super::expand;

    #[test]
    fn checks_command_calls_only() {
        let function = syn::parse_quote! {
            fn dispatch() {
                root::clean::run();
                target::create::run();
                other::helper::run();
            }
        };
        let expanded = expand(Default::default(), function).unwrap().to_string();
        assert!(expanded.contains("root :: clean :: COMMAND_MARKER"));
        assert!(expanded.contains("target :: create :: COMMAND_MARKER"));
        assert!(!expanded.contains("other :: helper :: COMMAND_MARKER"));
    }

    #[test]
    fn rejects_arguments() {
        let function = syn::parse_quote!(
            fn dispatch() {}
        );
        assert_eq!(
            expand(quote::quote!(unexpected), function)
                .unwrap_err()
                .to_string(),
            "command_dispatch takes no arguments"
        );
    }
}
