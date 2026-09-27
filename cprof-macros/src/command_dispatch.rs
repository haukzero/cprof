use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Expr, ExprCall, ItemFn, parse_quote,
    visit_mut::{self, VisitMut},
};

#[derive(Default)]
struct CommandCalls {
    count: usize,
}

impl VisitMut for CommandCalls {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        let Expr::Call(call) = expression else {
            visit_mut::visit_expr_mut(self, expression);
            return;
        };
        visit_mut::visit_expr_call_mut(self, call);
        if is_command_call(call) {
            self.count += 1;
            let mut call = call.clone();
            let Expr::Path(path) = call.func.as_mut() else {
                unreachable!("command calls have a path")
            };
            path.path.segments.last_mut().unwrap().ident = format_ident!("__cprof_dispatch_run");
            *expression = parse_quote!(#call);
        }
    }
}

fn is_command_call(call: &ExprCall) -> bool {
    let Expr::Path(path) = call.func.as_ref() else {
        return false;
    };
    let segments = &path.path.segments;
    segments.len() == 3
        && (segments[0].ident == "root" || segments[0].ident == "target")
        && segments[2].ident == "run"
}

pub(crate) fn expand(args: TokenStream, mut function: ItemFn) -> syn::Result<TokenStream> {
    if !args.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            "command_dispatch takes no arguments",
        ));
    }
    let mut calls = CommandCalls::default();
    calls.visit_block_mut(&mut function.block);
    if calls.count == 0 {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "command_dispatch requires a command run call",
        ));
    }
    Ok(quote!(#function))
}

#[cfg(test)]
mod tests {
    use super::expand;

    #[test]
    fn routes_command_calls_through_generated_entries() {
        let function = syn::parse_quote! {
            fn dispatch() {
                root::clean::run();
                target::create::run();
                other::helper::run();
            }
        };
        let expanded = expand(Default::default(), function).unwrap().to_string();
        assert!(expanded.contains("root :: clean :: __cprof_dispatch_run ()"));
        assert!(expanded.contains("target :: create :: __cprof_dispatch_run ()"));
        assert!(expanded.contains("other :: helper :: run ()"));
    }

    #[test]
    fn rejects_arguments_or_empty_dispatch() {
        let function: syn::ItemFn = syn::parse_quote!(
            fn dispatch() {}
        );
        assert_eq!(
            expand(quote::quote!(unexpected), function.clone())
                .unwrap_err()
                .to_string(),
            "command_dispatch takes no arguments"
        );
        assert_eq!(
            expand(Default::default(), function)
                .unwrap_err()
                .to_string(),
            "command_dispatch requires a command run call"
        );
    }
}
