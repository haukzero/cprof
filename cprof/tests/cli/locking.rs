use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::support::{AUTH, CONFIG, TestHome};

const BUSY: &str = "another cprof operation is active";
const TIMEOUT: Duration = Duration::from_secs(10);

// Reuse this test executable as a portable editor. The trailing argument is
// the draft path; libtest also accepts it as an additional exact test filter.
#[test]
fn editor_process() {
    let Ok(address) = env::var("CPROF_TEST_EDITOR_ADDRESS") else {
        return;
    };
    let draft = env::args_os().next_back().unwrap();
    fs::write(draft, env::var("CPROF_TEST_EDITOR_CONTENT").unwrap()).unwrap();
    let mut connection = TcpStream::connect(address).unwrap();
    connection.set_read_timeout(Some(TIMEOUT)).unwrap();
    connection.write_all(&[1]).unwrap();
    connection.read_exact(&mut [0]).unwrap();
}

struct PausedEdit {
    child: Option<Child>,
    connection: Option<TcpStream>,
}

impl PausedEdit {
    fn start(home: &TestHome, args: &[&str], content: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let child = home
            .command(args)
            .arg("--editor")
            .arg(env::current_exe().unwrap())
            .args([
                "--editor-arg=--exact",
                "--editor-arg=locking::editor_process",
                "--editor-arg=--nocapture",
            ])
            .env(
                "CPROF_TEST_EDITOR_ADDRESS",
                listener.local_addr().unwrap().to_string(),
            )
            .env("CPROF_TEST_EDITOR_CONTENT", content)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut edit = Self {
            child: Some(child),
            connection: None,
        };
        let deadline = Instant::now() + TIMEOUT;
        let mut connection = loop {
            match listener.accept() {
                Ok((connection, _)) => break connection,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("editor connection failed: {error}"),
            }
            if let Some(status) = edit.child.as_mut().unwrap().try_wait().unwrap() {
                let output = edit.child.take().unwrap().wait_with_output().unwrap();
                panic!("editor exited before becoming ready ({status}): {output:?}");
            }
            assert!(Instant::now() < deadline, "editor did not become ready");
            thread::sleep(Duration::from_millis(10));
        };
        connection.set_read_timeout(Some(TIMEOUT)).unwrap();
        connection.read_exact(&mut [0]).unwrap();
        edit.connection = Some(connection);
        edit
    }

    fn finish(mut self) -> Output {
        self.connection.as_mut().unwrap().write_all(&[1]).unwrap();
        self.child.take().unwrap().wait_with_output().unwrap()
    }
}

impl Drop for PausedEdit {
    fn drop(&mut self) {
        // A failed assertion must not leave an editor or its parent running.
        self.connection.take();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn active_edit_blocks_target_writers_and_preserves_the_draft() {
    let source = TestHome::new();
    source.codex_profile("work");
    source.claude_profile("incoming");
    source.succeeds(&["pack"]);
    let package = source.path.join("cprof.pkg");
    let package = package.to_str().unwrap();
    let claude_package = source.path.join("claude.pkg");
    let claude_package = claude_package.to_str().unwrap();
    source.succeeds(&["claude", "pack", "--save", claude_package]);

    let home = TestHome::new();
    let profile = home.codex_profile("work");
    let other = home.claude_profile("kept");
    home.set_config("[local]\n");
    let edited = "model = 'pending edit'\n";
    let edit = PausedEdit::start(
        &home,
        &["codex", "edit", "work", "--filename", "config"],
        edited,
    );
    let draft = profile.join(".config.cprof-edit.toml");

    let commands: &[&[&str]] = &[
        &["codex", "edit", "work"],
        &["codex", "create", "new", "-c", "work"],
        &["codex", "adopt", "adopted"],
        &["codex", "switch", "work", "--force"],
        &["codex", "unlink", "--force"],
        &["codex", "rename", "work", "renamed"],
        &["codex", "remove", "work", "--force"],
        &["codex", "clean", "--force"],
        &["clean", "--force"],
        &["codex", "unpack", "--path", package, "--force"],
        &["unpack", "--path", package, "--mirror", "--force"],
        // Root mirror also removes targets absent from the package.
        &["unpack", "--path", claude_package, "--mirror", "--force"],
        &["remove", "claude/kept", "codex/work", "--force"],
        &["clean", "--extra-toml", "--force"],
    ];
    for args in commands {
        home.fails(args, BUSY);
        assert_eq!(fs::read_to_string(&draft).unwrap(), edited, "{args:?}");
        assert_eq!(
            fs::read_to_string(profile.join("config.toml")).unwrap(),
            CONFIG
        );
        assert_eq!(fs::read_to_string(profile.join("auth.json")).unwrap(), AUTH);
        assert!(other.is_dir(), "{args:?}");
        assert!(!home.path.join(".cprof/profiles/claude/incoming").exists());
        assert_eq!(fs::read_to_string(home.config_path()).unwrap(), "[local]\n");
    }
    // Read-only commands and another target's editor remain available.
    home.succeeds(&["codex", "list"]);
    home.succeeds(&["codex", "unpack", "--path", package, "--dry-run"]);
    home.succeeds(&["claude", "edit", "kept"]);
    // A target import locks only its selected target, even in a multi-target package.
    home.succeeds(&["claude", "unpack", "--path", package, "--force"]);

    let output = edit.finish();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(profile.join("config.toml")).unwrap(),
        edited
    );
    assert!(!draft.exists());

    home.succeeds(&["codex", "unpack", "--path", package, "--force"]);
    assert_eq!(
        fs::read_to_string(profile.join("config.toml")).unwrap(),
        CONFIG
    );
    home.succeeds(&["codex", "rename", "work", "renamed"]);
    home.succeeds(&["codex", "remove", "renamed", "--force"]);
    assert!(!profile.exists());
}

#[test]
fn configuration_edit_blocks_import_and_root_clean_before_any_changes() {
    let source = TestHome::new();
    let package = source.pack_external_target("[incoming]\n", "incoming");
    let home = TestHome::new();
    home.set_config("[local]\n");
    let profile = home.codex_profile("kept");
    let edit = PausedEdit::start(&home, &["edit-extra"], "[edited]\n");
    let draft = home.path.join(".cprof/.extra-target.cprof-edit.toml");

    home.fails(&["edit-extra"], BUSY);
    home.fails(
        &["unpack", "--path", package.to_str().unwrap(), "--force"],
        BUSY,
    );
    home.fails(&["clean", "--extra-toml", "--force"], BUSY);
    assert!(profile.is_dir());
    assert!(!home.path.join(".cprof/profiles/incoming").exists());
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), "[local]\n");
    assert_eq!(fs::read_to_string(draft).unwrap(), "[edited]\n");

    let output = edit.finish();
    assert!(output.status.success(), "{output:?}");
    home.succeeds(&["unpack", "--path", package.to_str().unwrap(), "--force"]);
    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(config.contains("[edited]"));
    assert!(config.contains("[incoming]"));
}

#[test]
fn initial_profile_edit_holds_the_lock_until_creation_finishes() {
    let home = TestHome::new();
    home.claude_profile("source");
    // Existing unmanaged configuration prevents automatic activation, keeping
    // this lock test independent of Windows symlink privileges.
    fs::create_dir(home.path.join(".claude")).unwrap();
    fs::write(home.path.join(".claude/settings.json"), "{}\n").unwrap();
    let edit = PausedEdit::start(
        &home,
        &["claude", "create", "new", "-c", "source"],
        "{\"edited\":true}\n",
    );
    home.fails(&["claude", "clean", "--force"], BUSY);
    home.fails(&["claude", "rename", "new", "renamed"], BUSY);
    let output = edit.finish();
    assert!(output.status.success(), "{output:?}");
    home.succeeds(&["claude", "rename", "new", "renamed"]);
    assert_eq!(
        fs::read_to_string(
            home.path
                .join(".cprof/profiles/claude/renamed/settings.json")
        )
        .unwrap(),
        "{\"edited\":true}\n"
    );
}

#[test]
fn failed_import_releases_configuration_and_target_locks() {
    let source = TestHome::new();
    source.codex_profile("incoming");
    source.succeeds(&["pack"]);
    let home = TestHome::new();
    let profile = home.codex_profile("kept");
    // Planning reaches the locked target before discovering this invalid path.
    fs::write(profile.parent().unwrap().join("incoming"), "blocks profile").unwrap();
    let output = home.run(&[
        "unpack",
        "--path",
        source.path.join("cprof.pkg").to_str().unwrap(),
        "--force",
    ]);
    assert!(!output.status.success(), "{output:?}");
    home.succeeds(&["edit-extra"]);
    home.succeeds(&["codex", "edit", "kept"]);
}
