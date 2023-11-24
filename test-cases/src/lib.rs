use std::path::{Path, PathBuf};

pub fn get_test_case_file_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("cases")
        .join(filename)
}

pub fn load_test_case_file(filename: &str) -> String {
    let fixture_path = get_test_case_file_path(filename);
    std::fs::read_to_string(fixture_path).expect("Failed to read the fixture file")
}
