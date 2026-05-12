use crate::dev_preview::export::PRODUCER;
use crate::error::{GlorpError, Result};
use serde::Deserialize;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const OWNERSHIP_MARKER: &str = ".glorp-preview";

#[derive(Debug)]
pub struct PreparedOutput {
    pub final_dir: PathBuf,
    pub staging_dir: PathBuf,
}

pub fn prepare_output(final_dir: &Path) -> Result<PreparedOutput> {
    validate_replace_target(final_dir)?;

    let parent = final_dir
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let staging_dir = create_unique_staging_dir(parent, final_dir)?;

    Ok(PreparedOutput {
        final_dir: final_dir.to_path_buf(),
        staging_dir,
    })
}

pub fn commit_output(prepared: PreparedOutput) -> Result<()> {
    fs::write(prepared.staging_dir.join(OWNERSHIP_MARKER), PRODUCER)?;
    validate_replace_target(&prepared.final_dir)?;

    if prepared.final_dir.exists() {
        fs::remove_dir_all(&prepared.final_dir)?;
    }
    fs::rename(&prepared.staging_dir, &prepared.final_dir).map_err(|err| {
        GlorpError::Message(format!(
            "failed to replace {}: {err}",
            prepared.final_dir.display()
        ))
    })?;
    Ok(())
}

fn create_unique_staging_dir(parent: &Path, final_dir: &Path) -> Result<PathBuf> {
    let output_name = final_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("glorp-preview");
    let process_id = std::process::id();
    let timestamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for counter in 0..1000_u16 {
        let staging_dir = parent.join(format!(
            ".{output_name}.tmp-{process_id}-{timestamp_nanos}-{counter}"
        ));
        match fs::create_dir(&staging_dir) {
            Ok(()) => return Ok(staging_dir),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err.into()),
        }
    }

    Err(message(format!(
        "failed to create unique staging directory for {}",
        final_dir.display()
    )))
}

fn validate_replace_target(final_dir: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(final_dir) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(message(format!(
            "refusing to replace symlink output path {}",
            final_dir.display()
        )));
    }
    if file_type.is_file() {
        return Err(message(format!(
            "refusing to replace regular file output path {}",
            final_dir.display()
        )));
    }
    if !file_type.is_dir() {
        return Err(message(format!(
            "refusing to replace non-directory output path {}",
            final_dir.display()
        )));
    }

    if fs::read_dir(final_dir)?.next().is_none() {
        return Ok(());
    }

    validate_owned_preview_dir(final_dir)
}

fn validate_owned_preview_dir(final_dir: &Path) -> Result<()> {
    let marker_path = final_dir.join(OWNERSHIP_MARKER);
    if !marker_path.is_file() {
        return Err(message(format!(
            "refusing to replace non-empty directory not owned by glorp preview: missing {} in {}",
            OWNERSHIP_MARKER,
            final_dir.display()
        )));
    }

    let manifest_path = final_dir.join("manifest.json");
    let manifest: ExistingManifest = serde_json::from_str(&fs::read_to_string(&manifest_path)?)
        .map_err(|err| {
            GlorpError::Message(format!(
                "refusing to replace preview directory with invalid manifest producer in {}: {err}",
                manifest_path.display()
            ))
        })?;
    if manifest.producer != PRODUCER {
        return Err(message(format!(
            "refusing to replace preview directory with producer {:?}; expected {PRODUCER}",
            manifest.producer
        )));
    }

    Ok(())
}

fn message(message: String) -> GlorpError {
    GlorpError::Message(message)
}

#[derive(Deserialize)]
struct ExistingManifest {
    producer: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev_preview::export::PRODUCER;
    use std::fs;

    fn write_owned_manifest(dir: &Path, producer: &str) {
        fs::write(dir.join(OWNERSHIP_MARKER), PRODUCER).unwrap();
        fs::write(
            dir.join("manifest.json"),
            format!(r#"{{"schema_version":1,"producer":"{producer}"}}"#),
        )
        .unwrap();
    }

    #[test]
    fn prepare_output_allows_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let final_dir = dir.path().join("preview");

        let prepared = prepare_output(&final_dir).unwrap();

        assert_eq!(prepared.final_dir, final_dir);
        assert!(prepared.staging_dir.is_dir());
        assert!(!prepared.final_dir.exists());
    }

    #[test]
    fn prepare_output_allows_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let final_dir = dir.path().join("preview");
        fs::create_dir(&final_dir).unwrap();

        let prepared = prepare_output(&final_dir).unwrap();

        assert!(prepared.staging_dir.is_dir());
    }

    #[test]
    fn prepare_output_allows_owned_preview_directory() {
        let dir = tempfile::tempdir().unwrap();
        let final_dir = dir.path().join("preview");
        fs::create_dir(&final_dir).unwrap();
        write_owned_manifest(&final_dir, PRODUCER);
        fs::write(final_dir.join("stale.txt"), "stale").unwrap();

        let prepared = prepare_output(&final_dir).unwrap();

        assert!(prepared.staging_dir.is_dir());
    }

    #[test]
    fn prepare_output_keeps_existing_staging_dir_for_same_final_dir() {
        let dir = tempfile::tempdir().unwrap();
        let final_dir = dir.path().join("preview");

        let first = prepare_output(&final_dir).unwrap();
        fs::write(first.staging_dir.join("sentinel.txt"), "keep me").unwrap();

        let second = prepare_output(&final_dir).unwrap();

        assert_ne!(first.staging_dir, second.staging_dir);
        assert_eq!(
            fs::read_to_string(first.staging_dir.join("sentinel.txt")).unwrap(),
            "keep me"
        );
        assert!(second.staging_dir.is_dir());
    }

    #[test]
    fn prepare_output_refuses_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let final_dir = dir.path().join("preview");
        fs::write(&final_dir, "not a directory").unwrap();

        let err = prepare_output(&final_dir).unwrap_err().to_string();

        assert!(err.contains("regular file"), "{err}");
    }

    #[test]
    fn prepare_output_refuses_non_preview_directory() {
        let dir = tempfile::tempdir().unwrap();
        let final_dir = dir.path().join("preview");
        fs::create_dir(&final_dir).unwrap();
        fs::write(final_dir.join("user-file.txt"), "mine").unwrap();

        let err = prepare_output(&final_dir).unwrap_err().to_string();

        assert!(err.contains("not owned"), "{err}");
        assert!(final_dir.join("user-file.txt").exists());
    }

    #[test]
    fn prepare_output_refuses_preview_directory_with_wrong_producer() {
        let dir = tempfile::tempdir().unwrap();
        let final_dir = dir.path().join("preview");
        fs::create_dir(&final_dir).unwrap();
        write_owned_manifest(&final_dir, "someone-else");

        let err = prepare_output(&final_dir).unwrap_err().to_string();

        assert!(err.contains("producer"), "{err}");
    }

    #[test]
    fn commit_output_replaces_owned_directory() {
        let dir = tempfile::tempdir().unwrap();
        let final_dir = dir.path().join("preview");
        fs::create_dir(&final_dir).unwrap();
        write_owned_manifest(&final_dir, PRODUCER);
        fs::write(final_dir.join("stale.txt"), "stale").unwrap();

        let prepared = prepare_output(&final_dir).unwrap();
        fs::write(prepared.staging_dir.join("fresh.txt"), "fresh").unwrap();

        commit_output(prepared).unwrap();

        assert!(final_dir.join(OWNERSHIP_MARKER).is_file());
        assert!(final_dir.join("fresh.txt").is_file());
        assert!(!final_dir.join("stale.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn prepare_output_refuses_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let final_dir = dir.path().join("preview");
        fs::create_dir(&target).unwrap();
        symlink(&target, &final_dir).unwrap();

        let err = prepare_output(&final_dir).unwrap_err().to_string();

        assert!(err.contains("symlink"), "{err}");
    }
}
