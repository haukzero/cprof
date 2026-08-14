use crate::error::Result;
use crate::profile;
use crate::style;

pub fn run() -> Result<()> {
    match profile::get_settings_status()? {
        profile::SettingsStatus::Active(name) => println!("{}", name),
        profile::SettingsStatus::NoFile => {
            println!(
                "{}",
                style::warning("No active profile (settings.json does not exist)")
            );
        }
        profile::SettingsStatus::NotManaged => {
            println!(
                "{}",
                style::warning("settings.json exists but is not managed by cprof")
            );
        }
        profile::SettingsStatus::ExternalSymlink(target) => {
            println!(
                "{}",
                style::warning(&format!(
                    "settings.json is a symlink to an external path: {}",
                    target
                ))
            );
        }
    }
    Ok(())
}
