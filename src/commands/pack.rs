use std::fs;
use std::path::Path;

use crate::error::{AppError, Result};
use crate::package;
use crate::profile;

pub fn run(save: Option<String>) -> Result<()> {
    let output_path = save.unwrap_or_else(|| "cprof.pkg".to_string());
    let output = Path::new(&output_path);

    let profiles = profile::list_profiles()?;
    if profiles.is_empty() {
        return Err(AppError::Other("No profiles to pack".to_string()));
    }

    // Collect profiles that can be read (skip any that disappear during iteration)
    let mut entries = Vec::new();
    for p in &profiles {
        match profile::read_profile(&p.name) {
            Ok(content) => entries.push(package::PackageEntry {
                name: p.name.clone(),
                content,
            }),
            Err(_) => continue, // Profile was removed between listing and reading
        }
    }

    // Encode and write
    let data = package::encode(&entries)?;
    fs::write(output, &data)?;

    println!(
        "Packed {} profile(s) to '{}'",
        entries.len(),
        output.display()
    );

    Ok(())
}
