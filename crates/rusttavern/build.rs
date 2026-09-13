use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // These are the frontend/resource directories that feed generated Rust artifacts.
    println!("cargo:rerun-if-changed=../../default/content");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-env-changed=RUSTTAVERN_BUILD_BRANCH");
    println!("cargo:rerun-if-env-changed=RUSTTAVERN_BUILD_REVISION");

    emit_git_build_metadata();

    if let Err(error) = generate_resource_artifacts() {
        panic!("Failed to generate resource artifacts: {}", error);
    }
}

fn emit_git_build_metadata() {
    let git_branch = normalize_git_branch(
        std::env::var("RUSTTAVERN_BUILD_BRANCH")
            .ok()
            .or_else(|| std::env::var("GITHUB_REF_NAME").ok())
            .or_else(|| run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"])),
    );

    let git_revision = normalize_git_value(
        std::env::var("RUSTTAVERN_BUILD_REVISION")
            .ok()
            .or_else(|| std::env::var("GITHUB_SHA").ok())
            .map(|sha| shorten_revision(&sha))
            .or_else(|| run_git_command(&["rev-parse", "--short=12", "HEAD"])),
    );

    println!(
        "cargo:rustc-env=RUSTTAVERN_GIT_BRANCH={}",
        git_branch.unwrap_or_default()
    );
    println!(
        "cargo:rustc-env=RUSTTAVERN_GIT_REVISION={}",
        git_revision.unwrap_or_default()
    );
}

fn run_git_command(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

fn normalize_git_value(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let normalized = value.trim();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized.to_string())
        }
    })
}

fn normalize_git_branch(value: Option<String>) -> Option<String> {
    let branch = normalize_git_value(value)?;
    if branch.eq_ignore_ascii_case("head") {
        None
    } else {
        Some(branch)
    }
}

fn shorten_revision(value: &str) -> String {
    value.trim().chars().take(12).collect()
}

fn generate_resource_artifacts() -> Result<(), Box<dyn Error>> {
    let content_root = PathBuf::from("../../default/content");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);

    let mut content_files = collect_relative_files(&content_root, &content_root)?;
    content_files.sort();

    let content_manifest = serde_json::to_string(&content_files)?;
    write_if_changed(
        &out_dir.join("default_content_manifest.json"),
        content_manifest.as_bytes(),
    )?;

    Ok(())
}

fn write_if_changed(path: &Path, contents: &[u8]) -> Result<(), Box<dyn Error>> {
    if fs::read(path)
        .map(|existing| existing == contents)
        .unwrap_or(false)
    {
        return Ok(());
    }

    fs::write(path, contents)?;
    Ok(())
}

fn collect_relative_files(root: &Path, current: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            files.extend(collect_relative_files(root, &path)?);
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            files.push(relative);
        }
    }

    Ok(files)
}
