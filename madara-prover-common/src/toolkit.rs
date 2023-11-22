use std::fs::File;
use std::path::Path;

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
