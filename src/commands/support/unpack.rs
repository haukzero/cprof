use crate::error::Result;
use crate::package::unpack::{
    ChangeKind, ChangeSubject, UnpackInteraction, UnpackPreview, UnpackReport,
};
use crate::ui::{prompt, style};

/// Interactive conflict policy for a normal unpack command.
pub(crate) struct Interactive {
    force: bool,
}

impl Interactive {
    pub(crate) fn new(force: bool) -> Self {
        Self { force }
    }
}

impl UnpackInteraction for Interactive {
    fn use_packaged_config(&mut self, conflict: &str) -> Result<bool> {
        Ok(self.force || prompt::confirm(conflict)?)
    }

    fn begin_target(&mut self, target: &str) {
        println!(
            "{}",
            style::heading(&format!("Unpacking target '{target}':"))
        );
    }

    fn overwrite_profile(&mut self, name: &str) -> Result<bool> {
        style::warning(format!("Profile '{name}' already exists - conflict!"));
        let overwrite = self.force || prompt::confirm(&format!("Overwrite '{name}' ?"))?;
        if !overwrite {
            println!("Skipped '{name}'");
        }
        Ok(overwrite)
    }

    fn remove_profile(&mut self, name: &str) -> Result<bool> {
        style::warning(format!(
            "Profile '{name}' is not in the package - remove it?"
        ));
        let remove = self.force || prompt::confirm(&format!("Remove '{name}' ?"))?;
        if !remove {
            println!("Kept '{name}'");
        }
        Ok(remove)
    }
}

/// Conflict policy for an unpack preview: accept all packaged replacements.
pub(crate) struct Preview;

impl UnpackInteraction for Preview {
    fn use_packaged_config(&mut self, _conflict: &str) -> Result<bool> {
        Ok(true)
    }

    fn begin_target(&mut self, _target: &str) {}

    fn overwrite_profile(&mut self, _name: &str) -> Result<bool> {
        Ok(true)
    }

    fn remove_profile(&mut self, _name: &str) -> Result<bool> {
        Ok(true)
    }
}

pub(crate) fn render_restore(report: &UnpackReport) {
    for target in &report.targets {
        if target.profiles.is_empty() && target.removed.is_empty() && target.active_change.is_none()
        {
            continue;
        }
        for name in &target.profiles {
            println!("Unpacked '{name}'");
        }
        for name in &target.removed {
            println!("Removed '{name}'");
        }
        if let Some(active_change) = &target.active_change {
            match &active_change.desired {
                Some(name) => println!("Activated '{name}'"),
                None => println!("Cleared active configuration"),
            }
        }
        println!(
            "{}",
            style::heading(&format!("Unpacked target '{}'", target.target))
        );
    }
}

pub(crate) fn render_preview(preview: &UnpackPreview) {
    let mut current_target = None;
    for change in &preview.changes {
        let target = match &change.subject {
            ChangeSubject::TargetDefinition { target }
            | ChangeSubject::Profile { target, .. }
            | ChangeSubject::ActiveProfile { target, .. } => target,
        };
        if current_target != Some(target) {
            println!("target '{target}':");
            current_target = Some(target);
        }
        let marker = match change.kind {
            ChangeKind::Added => style::added_marker(),
            ChangeKind::Modified => style::modified_marker(),
            ChangeKind::Deleted => style::deleted_marker(),
        };
        let subject = match &change.subject {
            ChangeSubject::TargetDefinition { .. } => "configuration".to_string(),
            ChangeSubject::Profile { profile, .. } => format!("profile '{profile}'"),
            ChangeSubject::ActiveProfile { profile, .. } => {
                format!("active profile '{profile}'")
            }
        };
        println!("  {marker} {subject}");
    }
}
