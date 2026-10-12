use crate::profile::activation::Status;

use super::UnpackPlan;

/// Describes the targets that were unpacked after a successful commit.
pub(crate) struct UnpackReport {
    pub(crate) targets: Vec<UnpackedTarget>,
}

pub(crate) struct UnpackPreview {
    pub(crate) changes: Vec<UnpackChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ChangeSubject {
    TargetDefinition { target: String },
    Profile { target: String, profile: String },
    ActiveProfile { target: String, profile: String },
    ActiveConfiguration { target: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnpackChange {
    pub(crate) kind: ChangeKind,
    pub(crate) subject: ChangeSubject,
}

pub(crate) struct UnpackedTarget {
    pub(crate) target: String,
    pub(crate) profiles: Vec<String>,
    pub(crate) removed: Vec<String>,
    pub(crate) active_change: Option<ActiveChange>,
    pub(crate) unchanged: usize,
    pub(crate) skipped: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActiveChange {
    Activate { previous: Status, profile: String },
    Clear { previous: Status },
}

impl UnpackReport {
    pub(crate) fn profile_count(&self) -> usize {
        self.targets
            .iter()
            .map(|target| target.profiles.len())
            .sum()
    }

    pub(crate) fn skipped_count(&self) -> usize {
        self.targets.iter().map(|target| target.skipped).sum()
    }

    pub(crate) fn unchanged_count(&self) -> usize {
        self.targets.iter().map(|target| target.unchanged).sum()
    }

    pub(crate) fn removed_count(&self) -> usize {
        self.targets.iter().map(|target| target.removed.len()).sum()
    }
}

impl UnpackPlan {
    pub(super) fn preview_report(&self) -> UnpackPreview {
        let mut changes = self.target_config_changes.clone();
        for plan in &self.targets {
            for profile in &plan.profiles {
                changes.push(UnpackChange {
                    kind: if profile.overwrite {
                        ChangeKind::Modified
                    } else {
                        ChangeKind::Added
                    },
                    subject: ChangeSubject::Profile {
                        target: plan.target.id.clone(),
                        profile: profile.package.name.clone(),
                    },
                });
            }
            for name in &plan.removed {
                changes.push(UnpackChange {
                    kind: ChangeKind::Deleted,
                    subject: ChangeSubject::Profile {
                        target: plan.target.id.clone(),
                        profile: name.clone(),
                    },
                });
            }
            if let Some(activation) = &plan.activation {
                changes.push(activation.change.preview(&plan.target.id));
            }
        }
        changes.sort_by(|left, right| {
            change_target(&left.subject)
                .cmp(change_target(&right.subject))
                .then(left.subject.cmp(&right.subject))
                .then(left.kind.cmp(&right.kind))
        });
        UnpackPreview { changes }
    }
}

impl ActiveChange {
    fn preview(&self, target: &str) -> UnpackChange {
        let (kind, profile) = match self {
            Self::Activate { previous, profile } => (
                if *previous == Status::NoFiles {
                    ChangeKind::Added
                } else {
                    ChangeKind::Modified
                },
                Some(profile.as_str()),
            ),
            Self::Clear { previous } => (ChangeKind::Deleted, previous.active_name()),
        };
        UnpackChange {
            kind,
            subject: match profile {
                Some(profile) => ChangeSubject::ActiveProfile {
                    target: target.to_string(),
                    profile: profile.to_string(),
                },
                None => ChangeSubject::ActiveConfiguration {
                    target: target.to_string(),
                },
            },
        }
    }
}

fn change_target(subject: &ChangeSubject) -> &str {
    match subject {
        ChangeSubject::TargetDefinition { target }
        | ChangeSubject::Profile { target, .. }
        | ChangeSubject::ActiveProfile { target, .. }
        | ChangeSubject::ActiveConfiguration { target } => target,
    }
}
