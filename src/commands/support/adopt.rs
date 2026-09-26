use crate::elevate;
use crate::profile::activation;
use crate::ui::style;

/// Keep migration advice at the presentation layer, including JSON commands'
/// stderr, rather than emitting it from activation queries used by the library.
pub(crate) fn warn_unmanaged(target_id: &str, status: &activation::Status) {
    if matches!(status, activation::Status::Unmanaged) && !elevate::is_elevated_child() {
        style::warning(format!(
            "Active paths for '{target_id}' contain unmanaged files or links; \
             use `cprof {target_id} adopt <name>` to preserve and manage the current configuration"
        ));
    }
}
