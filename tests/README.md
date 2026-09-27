# Tests

## Running

Run from the repository root:

```sh
cargo test --all-targets
```

## Windows privileges

Tests that require symlink creation use the shared marker before `#[test]`:

```rust
#[test_macros::windows_elevation]
#[test]
fn restores_active_links() {
    // ...
}
```

The marker in [macros/lib.rs](macros/lib.rs) keeps tests enabled on Unix and
ignores them by default on Windows. Enable Windows Developer Mode or use an
elevated terminal, then run:

```sh
cargo test --all-targets -- --include-ignored
```

The marker does not launch UAC. Keep tests that verify missing-privilege failures
in the default suite.

## CLI fixture

[TestHome](cli/support/mod.rs) isolates configuration in a temporary directory.
On Windows, it uses `--elevated-home` to prevent UAC prompts and selects the
elevated-child output behavior. Tests of parent-only output use `#[cfg(unix)]`;
`--include-ignored` does not enable them on Windows.
