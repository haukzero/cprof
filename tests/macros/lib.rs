use proc_macro::TokenStream;

/// Place before `#[test]` for tests that require Windows symlink privileges.
/// Unix runs them normally; Windows opts in with `--include-ignored`.
#[proc_macro_attribute]
pub fn windows_elevation(args: TokenStream, test: TokenStream) -> TokenStream {
    assert!(args.is_empty(), "windows_elevation takes no arguments");
    let mut marked: TokenStream = r#"
        #[cfg_attr(
            windows,
            ignore = "requires symbolic-link privileges; run with --include-ignored"
        )]
    "#
    .parse()
    .unwrap();
    marked.extend(test);
    marked
}
