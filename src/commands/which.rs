use crate::error::Result;
use crate::profile;

pub fn run() -> Result<()> {
    match profile::get_active_name()? {
        Some(name) => println!("{}", name),
        None => println!("None"),
    }
    Ok(())
}
