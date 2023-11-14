/// Clones and builds the Stone Prover C++ repository to integrate it within
/// this crate.
extern crate git2;

use std::path::Path;

#[derive(Debug)]
enum CommandError {
    /// The command failed with a non-zero return code.
    CommandFailed(std::process::Output),
    /// The command could not be launched.
    IoError(std::io::Error),
}

impl From<std::io::Error> for CommandError {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}

/// Run any shell command line and retrieve its output.
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

/// Copy the prover and verifier binary files from the prover build container.
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

/// Build the Stone Prover and copy binaries to `output_dir`.
///
/// The prover repository contains a Dockerfile to build the prover. This function:
/// 1. Builds the Dockerfile
/// 2. Starts a container based on the generated image
/// 3. Extracts the binaries from the container
/// 4. Stops the container.
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

    // Stop the container
    let docker_delete_command = format!("docker rm {container_name}");
    run_command(&docker_delete_command).expect("Failed to stop and delete prover build container");

    // Handle a potential error during copy
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

/// Clone Git repository `repo_url` to directory `repo_clone_dir`.
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

    let prover_path = output_dir.join("cpu_air_prover");
    let verifier_path = output_dir.join("cpu_air_verifier");

    // Output the build information
    println!("cargo:rerun-if-changed={}", prover_path.to_string_lossy());
    println!("cargo:rerun-if-changed={}", verifier_path.to_string_lossy());
}
