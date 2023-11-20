use std::path::{Path, PathBuf};

pub fn get_fixture_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../stone-prover/tests/fixtures")
        .join(filename)
}

pub fn load_fixture(filename: &str) -> String {
    let fixture_path = get_fixture_path(filename);
    std::fs::read_to_string(fixture_path).expect("Failed to read the fixture file")
}
