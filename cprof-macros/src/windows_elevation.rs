use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{ItemFn, parse_quote};

pub(crate) fn expand(args: TokenStream, mut function: ItemFn) -> syn::Result<TokenStream> {
    if !args.is_empty() {
        return Err(syn::Error::new(
            Span::call_site(),
            "windows_elevation takes no arguments",
        ));
    }
    function.attrs.insert(
        0,
        parse_quote! {
            #[cfg_attr(
                windows,
                ignore = "requires symbolic-link privileges; run with --include-ignored"
            )]
        },
    );
    Ok(quote!(#function))
}

#[cfg(test)]
mod tests {
    use syn::{ItemFn, parse_quote};

    use super::expand;

    #[test]
    fn marks_windows_tests_without_changing_the_test() {
        let function = parse_quote!(
            #[test]
            fn requires_links() {}
        );
        let expanded: ItemFn = syn::parse2(expand(Default::default(), function).unwrap()).unwrap();
        assert!(expanded.attrs[0].path().is_ident("cfg_attr"));
        assert!(expanded.attrs[1].path().is_ident("test"));
        assert_eq!(expanded.sig.ident, "requires_links");
        assert_eq!(expanded.block.stmts.len(), 0);
    }

    #[test]
    fn rejects_arguments() {
        let function = parse_quote!(
            #[test]
            fn requires_links() {}
        );
        assert_eq!(
            expand(quote::quote!(unexpected), function)
                .unwrap_err()
                .to_string(),
            "windows_elevation takes no arguments"
        );
    }
}
