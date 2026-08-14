use crate::error::Result;
use crate::profile;

pub fn run() -> Result<()> {
    let profiles = profile::list_profiles()?;
    println!("{}", profiles.len());
    Ok(())
}
