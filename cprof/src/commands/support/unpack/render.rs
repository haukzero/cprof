use crate::package::unpack::{ChangeKind, ChangeSubject, UnpackPreview, UnpackReport};
use crate::ui::style;

pub(crate) fn render_restore(report: &UnpackReport) {
    for target in &report.targets {
        if target.profiles.is_empty() && target.removed.is_empty() && target.active_change.is_none()
        {
            continue;
        }
        println!(
            "{}",
            style::heading(&format!("Unpacked target '{}':", target.target))
        );
        for name in &target.profiles {
            println!("  Unpacked profile '{name}'");
        }
        for name in &target.removed {
            println!("  Removed profile '{name}'");
        }
        if let Some(active_change) = &target.active_change {
            match &active_change.desired {
                Some(name) => println!("  Activated profile '{name}'"),
                None => println!("  Cleared active configuration"),
            }
        }
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
