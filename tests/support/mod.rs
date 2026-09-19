use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

pub(crate) struct TestHome {
    pub(crate) path: PathBuf,
    _directory: tempfile::TempDir,
}

impl TestHome {
    pub(crate) fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        Self {
            path: directory.path().to_path_buf(),
            _directory: directory,
        }
    }

    pub(crate) fn config_path(&self) -> PathBuf {
        self.path.join(".cprof/extra-target.toml")
    }

    pub(crate) fn set_config(&self, content: &str) {
        fs::create_dir_all(self.path.join(".cprof")).unwrap();
        fs::write(self.config_path(), content).unwrap();
    }

    pub(crate) fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cprof"))
            .args(args)
            .env("HOME", &self.path)
            .env("NO_COLOR", "1")
            .current_dir(&self.path)
            .output()
            .unwrap()
    }

    pub(crate) fn succeeds(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(output.status.success(), "{args:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap()
    }

    pub(crate) fn fails(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(!output.status.success(), "{args:?}: {output:?}");
        String::from_utf8(output.stderr).unwrap()
    }

    pub(crate) fn pack_external_target(&self, config: &str, target_id: &str) -> PathBuf {
        self.set_config(config);
        fs::create_dir_all(
            self.path
                .join(".cprof/profiles")
                .join(target_id)
                .join("test"),
        )
        .unwrap();
        self.succeeds(&["pack"]);
        self.path.join("cprof.pkg")
    }
}
