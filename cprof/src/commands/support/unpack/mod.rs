use std::path::Path;

use crate::error::Result;
use crate::package::unpack::{
    self, UnpackMode, UnpackPlan, UnpackPreview, UnpackReport, UnpackSource,
};
use crate::targets::TargetRepository;

use super::write::WriteSession;

mod interaction;
mod render;
mod replay;

use interaction::{Interactive, Preview};
pub(crate) use render::{render_preview, render_restore};

/// Own the write session until commit finishes, including transaction recovery.
pub(crate) struct PreparedUnpack {
    plan: UnpackPlan,
    interaction: Interactive,
    write: WriteSession,
}

pub(crate) fn prepare(
    path: &Path,
    target_id: Option<&str>,
    force: bool,
    mirror: bool,
) -> Result<PreparedUnpack> {
    let mut write = WriteSession::configuration()?;
    let targets = TargetRepository::load()?;
    let source = UnpackSource::read(&targets, path, target_id, UnpackMode::from(mirror))?;
    write.include_targets(source.target_ids(&targets))?;
    let mut interaction = Interactive::new(force)?;
    let plan = source.prepare(&targets, &mut interaction)?;
    interaction.finish(&plan)?;
    Ok(PreparedUnpack {
        plan,
        interaction,
        write,
    })
}

pub(crate) fn preview(
    targets: &TargetRepository,
    path: &Path,
    target_id: Option<&str>,
    mirror: bool,
) -> Result<UnpackPreview> {
    unpack::preview(
        targets,
        path,
        target_id,
        UnpackMode::from(mirror),
        &mut Preview,
    )
}

impl PreparedUnpack {
    pub(crate) fn retry_context(&self) -> Result<serde_json::Value> {
        self.interaction.retry_context(&self.plan)
    }

    pub(crate) fn report(&self) -> UnpackReport {
        self.plan.report()
    }

    pub(crate) fn commit(self) -> Result<()> {
        self.write.run(|| self.plan.commit())
    }
}
