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
fn generates_a_checked_dispatch_entry() {
    let options = syn::parse_str::<Command>("no_retry").unwrap();
    let function = syn::parse_quote! {
        pub fn run() -> Result<()> { Ok(()) }
    };
    let expanded = expand(options, function).unwrap().to_string();
    assert!(expanded.contains("pub fn run () -> Result < () >"));
    assert!(expanded.contains("__cprof_dispatch_run () -> crate :: error :: Result < () >"));
    assert!(expanded.contains("let result : crate :: error :: Result < () > = run ()"));
}

#[test]
fn wraps_context_in_the_lazy_retry_callback() {
    let options = syn::parse_str(
        "retry = plan.commit, args = [\"unpack\"], context = interaction.retry_context()?",
    )
    .unwrap();
    let function = syn::parse_quote! {
        fn run() -> Result<()> { plan.commit()?; Ok(()) }
    };
    let expanded = expand(options, function).unwrap().to_string();
    assert!(expanded.contains("crate :: elevate :: run_with_context"));
    let compact: String = expanded
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert!(compact.contains("||Ok(Some(interaction.retry_context()?))"));
    assert!(
        syn::parse_str::<Command>("retry = update, args = [\"test\"], context = a, context = b")
            .is_err()
    );
}

#[test]
fn accepts_a_result_alias_for_the_compiler_to_check() {
    let options = syn::parse_str::<Command>("no_retry").unwrap();
    let function = syn::parse_quote! {
        pub fn run(value: &str) -> CommandResult<()> { Ok(()) }
    };
    let expanded = expand(options, function).unwrap().to_string();
    assert!(expanded.contains("pub fn run (value : & str) -> CommandResult < () >"));
    assert!(
        expanded.contains("let result : crate :: error :: Result < () > = run (__cprof_arg_0)")
    );
}
