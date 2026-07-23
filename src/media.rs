use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use uuid::Uuid;

use crate::{android::MediaFile, error::ArisaError, proto};

const DEFAULT_MIME: &str = "application/octet-stream";
const RETENTION: Duration = Duration::from_secs(600);

pub async fn store(
    directory: &Path,
    files: Vec<proto::MediaFile>,
) -> Result<Vec<MediaFile>, ArisaError> {
    if files.is_empty() {
        return Err(ArisaError::InvalidArgument(
            "at least one media file is required".to_string(),
        ));
    }

    let mut stored = Vec::with_capacity(files.len());
    for file in files {
        if file.data.is_empty() {
            remove_stored(&stored).await;
            return Err(ArisaError::InvalidArgument(
                "media files cannot be empty".to_string(),
            ));
        }
        let path = directory.join(Uuid::new_v4().to_string());
        if let Err(error) = tokio::fs::write(&path, file.data).await {
            remove_stored(&stored).await;
            return Err(ArisaError::Internal(format!(
                "failed to store media file: {error}"
            )));
        }
        stored.push(MediaFile {
            path: path.to_string_lossy().into_owned(),
            mime: if file.mime.trim().is_empty() {
                DEFAULT_MIME.to_string()
            } else {
                file.mime
            },
            name: file.name,
        });
    }
    Ok(stored)
}

pub fn start_cleanup(directory: PathBuf) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(RETENTION);
        loop {
            interval.tick().await;
            cleanup(&directory).await;
        }
    });
}

async fn cleanup(directory: &Path) {
    let cutoff = SystemTime::now()
        .checked_sub(RETENTION)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let Ok(mut entries) = tokio::fs::read_dir(directory).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(metadata) = entry.metadata().await else {
            continue;
        };
        if metadata.modified().is_ok_and(|modified| modified < cutoff) {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
}

async fn remove_stored(files: &[MediaFile]) {
    for file in files {
        let _ = tokio::fs::remove_file(&file.path).await;
    }
}
