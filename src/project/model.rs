//! The `.gpsvideo` project document.
//!
//! Projects are human-readable JSON. Media is referenced (never embedded) by absolute path and,
//! when possible, a path relative to the project file so projects survive being moved together
//! with their media. Unknown fields are ignored and missing fields take defaults, so older and
//! newer versions can read each other's files.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::gauges::model::{Gauge, GaugeId};
use crate::telemetry::{SyncSettings, UnitSystem};
use crate::video::encoder::{RenderSettings, TimeRange};

pub const PROJECT_VERSION: u32 = 1;
pub const PROJECT_EXTENSION: &str = "gpsvideo";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaRef {
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative: Option<PathBuf>,
}

impl MediaRef {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            relative: None,
        }
    }

    /// Finds the media file: absolute path, then relative to the project, then by file name.
    pub fn resolve(&self, project_dir: Option<&Path>) -> Option<PathBuf> {
        if self.path.exists() {
            return Some(self.path.clone());
        }
        let dir = project_dir?;
        if let Some(rel) = &self.relative {
            let p = dir.join(rel);
            if p.exists() {
                return Some(p);
            }
        }
        let p = dir.join(self.path.file_name()?);
        p.exists().then_some(p)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreviewQuality {
    Draft,
    #[default]
    Half,
    Full,
}

impl PreviewQuality {
    pub const ALL: [PreviewQuality; 3] = [
        PreviewQuality::Draft,
        PreviewQuality::Half,
        PreviewQuality::Full,
    ];
    pub fn label(self) -> &'static str {
        match self {
            PreviewQuality::Draft => "Draft",
            PreviewQuality::Half => "Half",
            PreviewQuality::Full => "Full",
        }
    }
    /// Fraction of the display resolution to decode at.
    pub fn factor(self) -> f32 {
        match self {
            PreviewQuality::Draft => 0.35,
            PreviewQuality::Half => 0.6,
            PreviewQuality::Full => 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PreviewSettings {
    pub quality: PreviewQuality,
    pub use_proxy: bool,
}

impl Default for PreviewSettings {
    fn default() -> Self {
        Self {
            quality: PreviewQuality::Half,
            use_proxy: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimelineSettings {
    /// Visible fraction multiplier (1 = whole video).
    pub zoom: f32,
    pub scroll: f64,
}

impl Default for TimelineSettings {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            scroll: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub version: u32,
    pub name: String,
    pub video: Option<MediaRef>,
    pub telemetry: Option<MediaRef>,
    pub sync: SyncSettings,
    pub units: UnitSystem,
    pub gauges: Vec<Gauge>,
    pub next_gauge_id: u64,
    pub next_group_id: u32,
    pub render: RenderSettings,
    pub timeline: TimelineSettings,
    pub preview: PreviewSettings,
    /// Section of the video to render (timeline in/out points); `None` = whole video.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim: Option<TimeRange>,
    /// Name of the template the layout started from; names saved custom templates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            version: PROJECT_VERSION,
            name: "Untitled".into(),
            video: None,
            telemetry: None,
            sync: SyncSettings::default(),
            units: UnitSystem::Metric,
            gauges: Vec::new(),
            next_gauge_id: 1,
            next_group_id: 1,
            render: RenderSettings::default(),
            timeline: TimelineSettings::default(),
            preview: PreviewSettings::default(),
            trim: None,
            template: None,
        }
    }
}

impl Project {
    pub fn alloc_gauge_id(&mut self) -> GaugeId {
        let used = self.gauges.iter().map(|g| g.id.0).max().unwrap_or(0);
        self.next_gauge_id = self.next_gauge_id.max(used + 1);
        let id = GaugeId(self.next_gauge_id);
        self.next_gauge_id += 1;
        id
    }

    pub fn gauge(&self, id: GaugeId) -> Option<&Gauge> {
        self.gauges.iter().find(|g| g.id == id)
    }

    pub fn gauge_mut(&mut self, id: GaugeId) -> Option<&mut Gauge> {
        self.gauges.iter_mut().find(|g| g.id == id)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("Unable to read the project file.")]
    Io(#[from] std::io::Error),
    #[error("The project file is damaged or not a TelemetryVibe project.")]
    Format(#[from] serde_json::Error),
    #[error("This project was created by a newer version of TelemetryVibe (format {0}).")]
    TooNew(u32),
}

fn relative_to(path: &Path, base: &Path) -> Option<PathBuf> {
    path.strip_prefix(base).ok().map(Path::to_path_buf)
}

/// Serializes a project to JSON, filling relative media paths for `project_path`.
pub fn to_json(project: &Project, project_path: Option<&Path>) -> Result<String, ProjectError> {
    let mut p = project.clone();
    p.version = PROJECT_VERSION;
    if let Some(dir) = project_path.and_then(Path::parent) {
        for m in [&mut p.video, &mut p.telemetry].into_iter().flatten() {
            m.relative = relative_to(&m.path, dir);
        }
    }
    Ok(serde_json::to_string_pretty(&p)?)
}

pub fn from_json(json: &str) -> Result<Project, ProjectError> {
    let p: Project = serde_json::from_str(json)?;
    if p.version > PROJECT_VERSION {
        return Err(ProjectError::TooNew(p.version));
    }
    Ok(p)
}

/// Writes atomically (temp file + rename) so a crash never leaves a truncated project.
pub fn save(project: &Project, path: &Path) -> Result<(), ProjectError> {
    let json = to_json(project, Some(path))?;
    let tmp = path.with_extension("gpsvideo.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    tracing::info!(path = %path.display(), "project saved");
    Ok(())
}

pub fn load(path: &Path) -> Result<Project, ProjectError> {
    let json = std::fs::read_to_string(path)?;
    let mut p = from_json(&json)?;
    let dir = path.parent();
    for m in [&mut p.video, &mut p.telemetry].into_iter().flatten() {
        if let Some(found) = m.resolve(dir) {
            m.path = found;
        }
    }
    tracing::info!(path = %path.display(), gauges = p.gauges.len(), "project loaded");
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gauges::library::{PresetId, make_preset};

    fn sample() -> Project {
        let mut p = Project {
            name: "Ride".into(),
            video: Some(MediaRef::new("/media/ride/clip.mp4")),
            telemetry: Some(MediaRef::new("/media/ride/ride.fit")),
            ..Default::default()
        };
        p.sync.offset = -12.8;
        p.units = UnitSystem::Imperial;
        p.trim = Some(TimeRange {
            start: 4.25,
            end: 31.5,
        });
        p.template = Some("Cycling".into());
        for preset in PresetId::ALL {
            let id = p.alloc_gauge_id();
            p.gauges.push(make_preset(
                *preset,
                id,
                (1920.0, 1080.0),
                None,
                UnitSystem::Imperial,
            ));
        }
        p
    }

    #[test]
    fn roundtrip() {
        let p = sample();
        let json = to_json(&p, Some(Path::new("/media/ride/project.gpsvideo"))).unwrap();
        let back = from_json(&json).unwrap();
        assert_eq!(back.gauges, p.gauges);
        assert_eq!(back.sync, p.sync);
        assert_eq!(back.units, p.units);
        assert_eq!(back.render, p.render);
        assert_eq!(back.trim, p.trim);
        assert_eq!(back.template, p.template);
        assert_eq!(
            back.video.unwrap().relative,
            Some(PathBuf::from("clip.mp4"))
        );
    }

    #[test]
    fn forward_compatible() {
        // Unknown fields are ignored, missing fields default.
        let json = r#"{"version":1,"name":"x","future_field":42,"gauges":[]}"#;
        let p = from_json(json).unwrap();
        assert_eq!(p.name, "x");
        assert_eq!(p.units, UnitSystem::Metric);
        assert_eq!(p.trim, None);
        assert!(from_json(r#"{"version":99}"#).is_err());
        assert!(from_json("not json").is_err());
    }

    #[test]
    fn save_and_load_file() {
        let dir = std::env::temp_dir().join(format!("telemetryvibe-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let media = dir.join("clip.mp4");
        std::fs::write(&media, b"x").unwrap();
        let mut p = sample();
        p.video = Some(MediaRef::new(&media));
        let path = dir.join("p.gpsvideo");
        save(&p, &path).unwrap();
        let back = load(&path).unwrap();
        assert_eq!(back.video.unwrap().path, media);
        assert_eq!(back.gauges.len(), p.gauges.len());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ids_are_unique() {
        let mut p = sample();
        let id = p.alloc_gauge_id();
        assert!(p.gauges.iter().all(|g| g.id != id));
    }
}
