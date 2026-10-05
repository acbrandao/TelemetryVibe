//! Autosave, crash recovery, recent projects and application settings.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::model::{Project, to_json};
use crate::utils::paths::AppPaths;

/// Autosave every five minutes, and shortly after major edits.
pub const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(300);
pub const MAJOR_CHANGE_DELAY: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryMeta {
    pub project_path: Option<PathBuf>,
    pub saved_at: chrono::DateTime<chrono::Utc>,
}

/// A recoverable autosave left behind by a session that did not exit cleanly.
#[derive(Clone, Debug)]
pub struct Recovery {
    pub file: PathBuf,
    pub meta: RecoveryMeta,
}

fn lock_file(paths: &AppPaths) -> PathBuf {
    paths.data_dir.join("session.lock")
}

fn recovery_file(paths: &AppPaths) -> PathBuf {
    paths.autosave_dir().join("recovery.gpsvideo")
}

fn recovery_meta(paths: &AppPaths) -> PathBuf {
    paths.autosave_dir().join("recovery.json")
}

/// Starts a session. Returns a recovery candidate if the previous session crashed.
pub fn begin_session(paths: &AppPaths) -> Option<Recovery> {
    let crashed = lock_file(paths).exists();
    let _ = std::fs::write(lock_file(paths), std::process::id().to_string());
    if !crashed {
        return None;
    }
    let file = recovery_file(paths);
    let meta: RecoveryMeta =
        serde_json::from_str(&std::fs::read_to_string(recovery_meta(paths)).ok()?).ok()?;
    file.exists().then_some(Recovery { file, meta })
}

/// Clean shutdown: removes the lock and the recovery copy.
pub fn end_session(paths: &AppPaths) {
    let _ = std::fs::remove_file(lock_file(paths));
    discard_recovery(paths);
}

pub fn discard_recovery(paths: &AppPaths) {
    let _ = std::fs::remove_file(recovery_file(paths));
    let _ = std::fs::remove_file(recovery_meta(paths));
}

/// Decides when to autosave.
pub struct Autosaver {
    last_save: Instant,
    dirty_since: Option<Instant>,
    major_since: Option<Instant>,
}

impl Default for Autosaver {
    fn default() -> Self {
        Self {
            last_save: Instant::now(),
            dirty_since: None,
            major_since: None,
        }
    }
}

impl Autosaver {
    pub fn mark_dirty(&mut self, major: bool) {
        let now = Instant::now();
        self.dirty_since.get_or_insert(now);
        if major {
            self.major_since = Some(now);
        }
    }

    pub fn due(&self) -> bool {
        let Some(dirty) = self.dirty_since else {
            return false;
        };
        let interval_due = self.last_save.elapsed() >= AUTOSAVE_INTERVAL
            && dirty.elapsed() > Duration::from_secs(1);
        let major_due = self
            .major_since
            .is_some_and(|t| t.elapsed() >= MAJOR_CHANGE_DELAY);
        interval_due || major_due
    }

    /// Writes the recovery copy on a background thread.
    pub fn save(&mut self, paths: &AppPaths, project: &Project, project_path: Option<&Path>) {
        self.last_save = Instant::now();
        self.dirty_since = None;
        self.major_since = None;
        let json = match to_json(project, project_path) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("autosave serialization failed: {e}");
                return;
            }
        };
        let meta = RecoveryMeta {
            project_path: project_path.map(Path::to_path_buf),
            saved_at: chrono::Utc::now(),
        };
        let dir = paths.autosave_dir();
        let file = recovery_file(paths);
        let meta_file = recovery_meta(paths);
        std::thread::spawn(move || {
            let _ = std::fs::create_dir_all(&dir);
            let tmp = file.with_extension("tmp");
            let ok = std::fs::write(&tmp, json).and_then(|_| std::fs::rename(&tmp, &file));
            if let Err(e) = ok {
                tracing::warn!("autosave failed: {e}");
                return;
            }
            if let Ok(m) = serde_json::to_string(&meta) {
                let _ = std::fs::write(meta_file, m);
            }
            tracing::debug!("autosaved");
        });
    }
}

const MAX_RECENT: usize = 10;

pub fn load_recent(paths: &AppPaths) -> Vec<PathBuf> {
    std::fs::read_to_string(paths.config_dir.join("recent.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<PathBuf>>(&s).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.exists())
        .collect()
}

pub fn add_recent(paths: &AppPaths, recent: &mut Vec<PathBuf>, path: &Path) {
    recent.retain(|p| p != path);
    recent.insert(0, path.to_path_buf());
    recent.truncate(MAX_RECENT);
    if let Ok(s) = serde_json::to_string_pretty(recent) {
        let _ = std::fs::write(paths.config_dir.join("recent.json"), s);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Dark,
    Light,
    Nord,
    Dracula,
    PaleYellow,
    SolarizedDark,
    Gruvbox,
}

impl Theme {
    pub const ALL: [Theme; 7] = [
        Theme::Dark,
        Theme::Light,
        Theme::Nord,
        Theme::Dracula,
        Theme::PaleYellow,
        Theme::SolarizedDark,
        Theme::Gruvbox,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Theme::Dark => "Dark",
            Theme::Light => "Light",
            Theme::Nord => "Nord",
            Theme::Dracula => "Dracula",
            Theme::PaleYellow => "Pale Yellow",
            Theme::SolarizedDark => "Solarized Dark",
            Theme::Gruvbox => "Gruvbox",
        }
    }
}

/// Per-user application settings (not stored in projects).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub ffmpeg_path: Option<PathBuf>,
    pub theme: Theme,
    pub hardware_decode: bool,
    pub last_dir: Option<PathBuf>,
    pub auto_proxy: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            ffmpeg_path: None,
            theme: Theme::Dark,
            hardware_decode: true,
            last_dir: None,
            auto_proxy: true,
        }
    }
}

impl AppSettings {
    pub fn load(paths: &AppPaths) -> Self {
        std::fs::read_to_string(paths.config_dir.join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, paths: &AppPaths) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(paths.config_dir.join("settings.json"), s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autosave_timing() {
        let mut a = Autosaver::default();
        assert!(!a.due());
        a.mark_dirty(false);
        assert!(!a.due(), "minor edits wait for the interval");
        a.major_since = Some(Instant::now() - MAJOR_CHANGE_DELAY);
        assert!(a.due());
    }
}
