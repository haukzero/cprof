use crate::elevate;
use crate::error::Result;
use crate::package::unpack::{UnpackInteraction, UnpackPlan};
use crate::targets::external::ConfigConflict;
use crate::ui::{prompt, style};

use super::replay::{DecisionKey, ReplaySession};

/// Interactive policy for a normal unpack. The replay session is populated on
/// the first attempt and consumed by an elevated child.
pub(crate) struct Interactive {
    force: bool,
    session: ReplaySession,
}

impl Interactive {
    pub(crate) fn new(force: bool) -> Result<Self> {
        Ok(Self {
            force,
            session: ReplaySession::new(elevate::take_context()?)?,
        })
    }

    pub(crate) fn retry_context(&self, plan: &UnpackPlan) -> Result<serde_json::Value> {
        self.session.context(plan.fingerprint())
    }

    pub(crate) fn finish(&self, plan: &UnpackPlan) -> Result<()> {
        self.session.finish(plan.fingerprint())
    }

    fn confirm(&mut self, key: DecisionKey, prompt: &str) -> Result<bool> {
        self.session
            .decide(key, || Ok(self.force || prompt::confirm(prompt)?))
    }
}

impl UnpackInteraction for Interactive {
    fn use_packaged_config(&mut self, conflict: &ConfigConflict) -> Result<bool> {
        self.confirm(
            DecisionKey::Configuration(conflict.clone()),
            &conflict.to_string(),
        )
    }

    fn begin_target(&mut self, target: &str) {
        if !self.session.is_replay() {
            println!(
                "{}",
                style::heading(&format!("Preparing target '{target}'..."))
            );
        }
    }

    fn overwrite_profile(&mut self, target: &str, name: &str) -> Result<bool> {
        if !self.session.is_replay() {
            style::warning(format!("Profile '{name}' already exists - conflict!"));
        }
        let overwrite = self.confirm(
            DecisionKey::OverwriteProfile {
                target: target.to_owned(),
                profile: name.to_owned(),
            },
            &format!("Overwrite '{name}' ?"),
        )?;
        if !overwrite && !self.session.is_replay() {
            println!("Skipped profile '{name}'");
        }
        Ok(overwrite)
    }

    fn remove_profile(&mut self, target: &str, name: &str) -> Result<bool> {
        if !self.session.is_replay() {
            style::warning(format!(
                "Profile '{name}' is not in the package - remove it?"
            ));
        }
        let remove = self.confirm(
            DecisionKey::RemoveProfile {
                target: target.to_owned(),
                profile: name.to_owned(),
            },
            &format!("Remove '{name}' ?"),
        )?;
        if !remove && !self.session.is_replay() {
            println!("Kept profile '{name}'");
        }
        Ok(remove)
    }
}

/// Preview answers every conflict positively because it never commits changes.
pub(crate) struct Preview;

impl UnpackInteraction for Preview {
    fn use_packaged_config(&mut self, _: &ConfigConflict) -> Result<bool> {
        Ok(true)
    }
    fn begin_target(&mut self, _: &str) {}
    fn overwrite_profile(&mut self, _: &str, _: &str) -> Result<bool> {
        Ok(true)
    }
    fn remove_profile(&mut self, _: &str, _: &str) -> Result<bool> {
        Ok(true)
    }
}
