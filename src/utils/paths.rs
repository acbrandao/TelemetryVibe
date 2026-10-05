//! Platform application directories (cache, config, data, logs).

use std::path::PathBuf;

/// Resolved application directories. Created lazily on first use.
#[derive(Clone, Debug)]
pub struct AppPaths {
    pub cache_dir: PathBuf,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
}

/// Recursively copies a directory (used for the one-time settings migration).
fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

impl AppPaths {
    /// Uses the platform conventions (`~/Library/Caches/...` on macOS, `%LOCALAPPDATA%` on Windows).
    pub fn resolve() -> Self {
        let dirs = directories::ProjectDirs::from("net", "TelemetryVibe", "TelemetryVibe");
        let fallback = std::env::temp_dir().join("TelemetryVibe");
        let (cache_dir, config_dir, data_dir) = match &dirs {
            Some(d) => (
                d.cache_dir().to_path_buf(),
                d.config_dir().to_path_buf(),
                d.data_local_dir().to_path_buf(),
            ),
            None => (
                fallback.join("cache"),
                fallback.join("config"),
                fallback.join("data"),
            ),
        };
        // One-time move of settings, templates, recent files and autosaves from the app's
        // former names, newest first (cache is not copied; it regenerates).
        for old in ["TrackVibe", "GaugeRust"]
            .into_iter()
            .filter_map(|name| directories::ProjectDirs::from("net", name, name))
        {
            for (from, to) in [
                (old.config_dir(), config_dir.as_path()),
                (old.data_local_dir(), data_dir.as_path()),
            ] {
                if from.is_dir() && from != to && !to.exists() {
                    match copy_dir(from, to) {
                        Ok(()) => eprintln!(
                            "migrated {} -> {}",
                            from.display(),
                            to.display()
                        ),
                        Err(e) => eprintln!("could not migrate {}: {e}", from.display()),
                    }
                }
            }
        }
        let log_dir = data_dir.join("logs");
        let paths = Self {
            cache_dir,
            config_dir,
            data_dir,
            log_dir,
        };
        for dir in [
            &paths.cache_dir,
            &paths.config_dir,
            &paths.data_dir,
            &paths.log_dir,
        ] {
            let _ = std::fs::create_dir_all(dir);
        }
        paths
    }

    pub fn proxy_dir(&self) -> PathBuf {
        self.cache_dir.join("proxies")
    }

    pub fn thumbnail_dir(&self) -> PathBuf {
        self.cache_dir.join("thumbnails")
    }

    pub fn probe_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("probe")
    }

    pub fn autosave_dir(&self) -> PathBuf {
        self.data_dir.join("autosave")
    }

    /// Total size of the cache directory in bytes.
    pub fn cache_size(&self) -> u64 {
        dir_size(&self.cache_dir)
    }

    /// Removes all cached proxies, thumbnails and metadata.
    pub fn clear_cache(&self) -> std::io::Result<()> {
        if self.cache_dir.exists() {
            std::fs::remove_dir_all(&self.cache_dir)?;
        }
        std::fs::create_dir_all(&self.cache_dir)
    }
}

fn dir_size(path: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

/// A stable cache key for a media file based on path, size and modification time.
pub fn file_cache_key(path: &std::path::Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    if let Ok(meta) = std::fs::metadata(path) {
        meta.len().hash(&mut h);
        if let Ok(modified) = meta.modified()
            && let Ok(d) = modified.duration_since(std::time::UNIX_EPOCH)
        {
            d.as_millis().hash(&mut h);
        }
    }
    format!("{:016x}", h.finish())
}
