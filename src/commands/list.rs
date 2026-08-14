use crate::error::Result;
use crate::profile;
use crate::style;

pub fn run() -> Result<()> {
    let profiles = profile::list_profiles()?;

    if profiles.is_empty() {
        println!("No profiles found.");
        return Ok(());
    }

    for p in &profiles {
        if p.active {
            println!("{} {}", style::active_name(&p.name), style::active_tag());
        } else {
            println!("{}", p.name);
        }
    }

    println!(
        "---------------------\n{} total",
        style::label(&profiles.len().to_string())
    );

    Ok(())
}
