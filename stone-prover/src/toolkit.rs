#[cfg(test)]
use std::fs;
use std::fs::File;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::Serialize;

pub fn read_json_from_file<T: DeserializeOwned, P: AsRef<Path>>(
    path: P,
) -> Result<T, std::io::Error> {
    let file = File::open(path)?;
    let mut reader = std::io::BufReader::new(file);

    let obj: T = serde_json::from_reader(&mut reader)?;
    Ok(obj)
}

pub fn write_json_to_file<T: Serialize, P: AsRef<Path>>(
    obj: T,
    path: P,
) -> Result<(), std::io::Error> {
    let mut file = File::create(path)?;
    serde_json::to_writer(&mut file, &obj)?;
    Ok(())
}

#[cfg(test)]
pub fn get_fixture_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(filename)
}

#[cfg(test)]
pub fn load_fixture(filename: &str) -> String {
    let fixture_path = get_fixture_path(filename);
    fs::read_to_string(fixture_path).expect("Failed to read the fixture file")
}
