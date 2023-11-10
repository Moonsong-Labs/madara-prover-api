extern crate git2;

use std::path::Path;

#[derive(Debug)]
enum CommandError {
    CommandFailed(std::process::Output),
    IoError(std::io::Error),
}

impl From<std::io::Error> for CommandError {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}

fn run_command(command: &str) -> Result<std::process::Output, CommandError> {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()?;

    if !output.status.success() {
        return Err(CommandError::CommandFailed(output));
    }
    Ok(output)
}

/// Copy a file from a running Docker container.
fn copy_file_from_container(
    container_name: &str,
    container_file: &Path,
    target: &Path,
) -> Result<(), CommandError> {
    let docker_copy_command = format!(
        "docker cp -L {container_name}:{} {}",
        container_file.to_string_lossy(),
        target.to_string_lossy()
    );
    let _ = run_command(&docker_copy_command);
    Ok(())
}

fn copy_prover_files_from_container(
    container_name: &str,
    output_dir: &Path,
) -> Result<(), CommandError> {
    copy_file_from_container(container_name, Path::new("/bin/cpu_air_prover"), output_dir)?;
    copy_file_from_container(
        container_name,
        Path::new("/bin/cpu_air_verifier"),
        output_dir,
    )?;

    Ok(())
}

fn build_stone_prover(repo_dir: &Path, output_dir: &Path) {
    // Build the Stone Prover build Docker image
    let image_name = "stone-prover-build:latest";
    let docker_build_command = format!(
        "docker build -t {image_name} {}",
        repo_dir.to_string_lossy()
    );
    run_command(&docker_build_command).expect("Failed to build Stone Prover using Dockerfile");

    // Run a container based on the Docker image
    let docker_create_command = format!("docker create {image_name}");
    let docker_create_output = run_command(&docker_create_command)
        .expect("Failed to start container to copy prover files");
    let container_name = String::from_utf8_lossy(&docker_create_output.stdout)
        .trim()
        .to_owned();
    println!("Started container {container_name}");

    // Copy the files
    let copy_result = copy_prover_files_from_container(&container_name, output_dir);

    let docker_delete_command = format!("docker rm {container_name}");
    run_command(&docker_delete_command).expect("Failed to stop and delete prover build container");

    if let Err(e) = copy_result {
        panic!(
            "Failed to copy files from the prover build container: {:?}",
            e
        );
    }
}

fn download_and_build_stone_prover(dependencies_dir: &Path, output_dir: &Path) {
    let repo_url = "https://github.com/starkware-libs/stone-prover";
    let repo_clone_dir = dependencies_dir.join("stone-prover");

    clone_repository(repo_url, &repo_clone_dir);

    build_stone_prover(&repo_clone_dir, output_dir);
}

fn clone_repository(repo_url: &str, repo_clone_dir: &Path) {
    if repo_clone_dir.exists() {
        println!("Repository already exists.");
    } else {
        let _ = git2::Repository::clone(repo_url, repo_clone_dir).unwrap();
        println!("Cloned repository to {}", repo_clone_dir.to_string_lossy());
    }
}

fn main() {
    let output_dir_str = &std::env::var_os("OUT_DIR").unwrap();
    let output_dir = Path::new(&output_dir_str);
    let dependencies_dir = Path::new("./dependencies");
    download_and_build_stone_prover(dependencies_dir, output_dir);

    // Output the build information
    println!("cargo:rerun-if-changed={output_dir_str}/cairo_air_prover");
    println!("cargo:rerun-if-changed={output_dir_str}/cairo_air_verifier");
}
