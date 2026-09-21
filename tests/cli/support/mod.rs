use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;

pub const CONFIG: &str = "# Keep comments and formatting\nmodel = 'example'\n";
pub const AUTH: &str = "{ \"example\": \"test credential\" }\n";

pub struct TestHome {
    pub path: PathBuf,
    editor: PathBuf,
    _directory: tempfile::TempDir,
}

impl TestHome {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("home with spaces");
        fs::create_dir(&path).unwrap();
        let editor = script(&path, "noop editor", "exit 0\n", "exit /b 0\n");
        Self {
            path,
            editor,
            _directory: directory,
        }
    }

    pub fn config_path(&self) -> PathBuf {
        self.path.join(".cprof/extra-target.toml")
    }

    pub fn set_config(&self, content: &str) {
        fs::create_dir_all(self.path.join(".cprof")).unwrap();
        fs::write(self.config_path(), content).unwrap();
    }

    pub fn write_profile(&self, target: &str, name: &str, resources: &[(&str, &str)]) -> PathBuf {
        let path = self.path.join(".cprof/profiles").join(target).join(name);
        fs::create_dir_all(&path).unwrap();
        for (filename, content) in resources {
            fs::write(path.join(filename), content).unwrap();
        }
        path
    }

    pub fn claude_profile(&self, name: &str) -> PathBuf {
        self.write_profile("claude", name, &[("settings.json", "{}\n")])
    }

    pub fn codex_profile(&self, name: &str) -> PathBuf {
        self.write_profile(
            "codex",
            name,
            &[("config.toml", CONFIG), ("auth.json", AUTH)],
        )
    }

    pub fn editor(&self, unix: &str, windows: &str) -> PathBuf {
        script(&self.path, "custom editor", unix, windows)
    }

    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cprof"));
        // Windows resolves HOME through the OS. The existing child prefix
        // isolates configuration and prevents unattended UAC prompts.
        #[cfg(windows)]
        command.arg("--elevated-home").arg(&self.path);
        command
            .args(args)
            .env("HOME", &self.path)
            .env("NO_COLOR", "1")
            .env_remove("VISUAL")
            .env(
                "EDITOR",
                shell_words::quote(self.editor.to_str().unwrap()).as_ref(),
            )
            .stdin(Stdio::null())
            .current_dir(&self.path);
        command
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    pub fn succeeds(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(output.status.success(), "{args:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap()
    }

    pub fn fails(&self, args: &[&str], message: &str) {
        assert_failure(&self.run(args), message);
    }

    pub fn pack_external_target(&self, config: &str, target: &str) -> PathBuf {
        self.set_config(config);
        fs::create_dir_all(self.path.join(".cprof/profiles").join(target).join("test")).unwrap();
        self.succeeds(&["pack"]);
        self.path.join("cprof.pkg")
    }
}

pub fn assert_failure(output: &Output, message: &str) {
    assert!(!output.status.success(), "expected {message:?}: {output:?}");
    // Windows child mode suppresses application errors but may print warnings.
    // Assert diagnostics on Unix; callers verify on-disk state on both systems.
    #[cfg(unix)]
    {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(message), "expected {message:?}: {stderr}");
    }
}

fn script(directory: &Path, name: &str, unix: &str, windows: &str) -> PathBuf {
    let (extension, content) = if cfg!(windows) {
        ("cmd", format!("@echo off\n{windows}").replace('\n', "\r\n"))
    } else {
        ("sh", format!("#!/bin/sh\nset -eu\n{unix}"))
    };
    let path = directory.join(name).with_extension(extension);
    fs::write(&path, content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    path
}

pub fn symlink(source: impl AsRef<Path>, link: impl AsRef<Path>) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, link)
    }
    #[cfg(windows)]
    if source.as_ref().is_dir() {
        std::os::windows::fs::symlink_dir(source, link)
    } else {
        std::os::windows::fs::symlink_file(source, link)
    }
}

pub fn can_symlink() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    let available = *AVAILABLE.get_or_init(|| {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("missing");
        let link = directory.path().join("link");
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(source, link);
        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_file(source, link);
        match result {
            Ok(()) => true,
            Err(error) if cfg!(windows) && error.raw_os_error() == Some(1314) => false,
            Err(error) => panic!("cannot probe symlink support: {error}"),
        }
    });
    if !available {
        eprintln!(
            "Symlink creation unavailable: link success cases require Windows Developer Mode or elevation"
        );
    }
    available
}

pub fn unmanaged_codex() -> TestHome {
    let home = TestHome::new();
    fs::create_dir(home.path.join(".codex")).unwrap();
    fs::write(home.path.join(".codex/config.toml"), CONFIG).unwrap();
    fs::write(home.path.join(".codex/auth.json"), AUTH).unwrap();
    home
}

pub fn assert_originals(home: &TestHome) {
    for (filename, content) in [("config.toml", CONFIG), ("auth.json", AUTH)] {
        let path = home.path.join(".codex").join(filename);
        assert!(!path.is_symlink());
        assert_eq!(fs::read_to_string(path).unwrap(), content);
    }
}

pub const INVALID_CONFIGS: &[(&str, &str)] = &[
    ("[broken", "Failed to parse"),
    ("[demo]\nid = 1\n", "Failed to parse"),
    ("[claude]\n", "conflicts with an existing target"),
    (
        "[edit-extra]\n",
        "conflicts with the command of the same name",
    ),
    ("[help]\n", "conflicts with the command of the same name"),
    (
        "[first]\nid = 'same'\n[second]\nid = 'same'\n",
        "conflicts with an existing target",
    ),
    (
        "[[demo.resources]]\nfilename = 'a'\nactive_path = '.demo/a'\n\
      [[demo.resources]]\nkey = 'a'\nfilename = 'b'\nactive_path = '.demo/b'\n",
        "declares duplicate resource",
    ),
];
