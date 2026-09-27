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
pub(crate) struct ActiveChange {
    pub(crate) previous: Option<String>,
    pub(crate) desired: Option<String>,
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
            if let Some(active_change) = &plan.active_change {
                let kind = match (&active_change.previous, &active_change.desired) {
                    (None, Some(_)) => ChangeKind::Added,
                    (Some(_), None) => ChangeKind::Deleted,
                    (Some(_), Some(_)) => ChangeKind::Modified,
                    (None, None) => unreachable!(),
                };
                let profile = active_change
                    .desired
                    .as_ref()
                    .or(active_change.previous.as_ref())
                    .expect("active change has a profile");
                changes.push(UnpackChange {
                    kind,
                    subject: ChangeSubject::ActiveProfile {
                        target: plan.target.id.clone(),
                        profile: profile.clone(),
                    },
                });
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

fn change_target(subject: &ChangeSubject) -> &str {
    match subject {
        ChangeSubject::TargetDefinition { target }
        | ChangeSubject::Profile { target, .. }
        | ChangeSubject::ActiveProfile { target, .. } => target,
    }
}
