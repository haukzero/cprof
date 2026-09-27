# Tests

## Running

Run from the repository root:

```sh
cargo test --workspace --all-targets
```

## Windows privileges

Tests that require symlink creation use the shared marker before `#[test]`:

```rust
#[cprof_macros::windows_elevation]
#[test]
fn restores_active_links() {
    // ...
}
```

The marker in [cprof-macros](../../cprof-macros/src/windows_elevation.rs) keeps
tests enabled on Unix and ignores them by default on Windows. Enable Windows
Developer Mode or use an elevated terminal, then run:

```sh
cargo test --workspace --all-targets -- --include-ignored
```

The marker does not launch UAC. Keep tests that verify missing-privilege failures
in the default suite.

The unpack decision replay tests are platform-independent and exercise the
decision keys without launching an elevated process. The context-file tests
under `elevate::context` compile only on Windows because they verify Windows
file-sharing behavior used by the UAC retry.

## CLI fixture

[TestHome](cli/support/mod.rs) isolates configuration in a temporary directory.
On Windows, it uses `--elevated-home` to prevent UAC prompts and selects the
elevated-child output behavior. Tests of parent-only output use `#[cfg(unix)]`;
`--include-ignored` does not enable them on Windows.
