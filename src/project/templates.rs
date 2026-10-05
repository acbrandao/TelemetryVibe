//! User templates: gauge layouts saved from a project and offered next to the built-in ones.
//!
//! Stored per user in `templates.json` in the config directory. A template keeps full gauge
//! definitions (kind, data source, range, style, zones, smoothing) plus the video size they were
//! laid out on, so it can be re-applied to videos of any resolution or aspect ratio.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::gauges::model::{Gauge, GaugeId};

/// Name used when the layout did not start from a template.
pub const DEFAULT_BASE: &str = "Custom Template";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserTemplate {
    pub name: String,
    /// Template the layout started from (used to name further copies).
    #[serde(default)]
    pub based_on: Option<String>,
    /// Video size (px) the gauges were laid out on.
    pub video_size: (f32, f32),
    pub gauges: Vec<Gauge>,
    #[serde(default = "chrono::Utc::now")]
    pub created: chrono::DateTime<chrono::Utc>,
}

impl UserTemplate {
    pub fn description(&self) -> String {
        let n = self.gauges.len();
        format!(
            "{n} gauge{} · saved {}",
            if n == 1 { "" } else { "s" },
            self.created
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
        )
    }

    /// Copies the gauges for a video of `video` size, with fresh ids and group ids.
    ///
    /// Sizes scale with the video height (like the presets). Each gauge keeps its distance to the
    /// nearest edge per axis, so corner and edge layouts stay aligned when the aspect ratio
    /// changes; gauges in the middle third keep their relative position.
    pub fn instantiate(
        &self,
        video: (f32, f32),
        next_id: &mut dyn FnMut() -> GaugeId,
        next_group: &mut dyn FnMut() -> u32,
    ) -> Vec<Gauge> {
        let (tw, th) = (self.video_size.0.max(1.0), self.video_size.1.max(1.0));
        let (vw, vh) = video;
        let k = vh / th;
        let mut groups: HashMap<u32, u32> = HashMap::new();
        self.gauges
            .iter()
            .map(|src| {
                let mut g = src.clone();
                g.id = next_id();
                g.group = src
                    .group
                    .map(|old| *groups.entry(old).or_insert_with(&mut *next_group));
                let p = &mut g.placement;
                let (cx, cy) = (p.x + p.w / 2.0, p.y + p.h / 2.0);
                let (w, h) = (p.w * k, p.h * k);
                p.x = place(cx, p.x, p.w, tw, vw, w, k);
                p.y = place(cy, p.y, p.h, th, vh, h, k);
                p.w = w;
                p.h = h;
                g.style.border_width *= k;
                g
            })
            .collect()
    }
}

/// New start coordinate on one axis (see [`UserTemplate::instantiate`]).
fn place(
    center: f32,
    start: f32,
    size: f32,
    src_len: f32,
    dst_len: f32,
    new_size: f32,
    k: f32,
) -> f32 {
    let f = center / src_len;
    let v = if f < 1.0 / 3.0 {
        start * k
    } else if f > 2.0 / 3.0 {
        let far_margin = src_len - (start + size);
        dst_len - far_margin * k - new_size
    } else {
        f * dst_len - new_size / 2.0
    };
    v.round()
}

/// Strips a " (Custom)" / " (Custom N)" suffix to find the root template name.
pub fn root_name(name: &str) -> &str {
    let n = name.trim();
    if let Some(open) = n.rfind(" (Custom")
        && n.ends_with(')')
    {
        let inner = &n[open + " (Custom".len()..n.len() - 1];
        if inner.is_empty() || inner.trim().parse::<u32>().is_ok() {
            return &n[..open];
        }
    }
    n
}

/// Next free automatic name: `Cycling (Custom)`, `Cycling (Custom 2)`, …
/// Layouts that did not start from a template become `Custom Template`, `Custom Template 2`, …
pub fn auto_name(based_on: Option<&str>, existing: &[UserTemplate]) -> String {
    let taken = |n: &str| existing.iter().any(|t| t.name.eq_ignore_ascii_case(n));
    // Copies of "Custom Template N" continue that numbering rather than nesting "(Custom)".
    let root = based_on
        .map(root_name)
        .filter(|r| !r.is_empty() && !r.starts_with(DEFAULT_BASE));
    let candidate = |i: u32| match (root, i) {
        (Some(r), 1) => format!("{r} (Custom)"),
        (Some(r), i) => format!("{r} (Custom {i})"),
        (None, 1) => DEFAULT_BASE.to_string(),
        (None, i) => format!("{DEFAULT_BASE} {i}"),
    };
    (1..).map(candidate).find(|n| !taken(n)).expect("unbounded")
}

#[derive(Debug, Default)]
pub struct TemplateStore {
    pub templates: Vec<UserTemplate>,
    path: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
struct TemplateFile {
    version: u32,
    templates: Vec<UserTemplate>,
}

impl TemplateStore {
    pub fn file_path(config_dir: &Path) -> PathBuf {
        config_dir.join("templates.json")
    }

    /// Loads the store; a missing or unreadable file gives an empty store.
    pub fn load(path: PathBuf) -> Self {
        let templates = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| match serde_json::from_str::<TemplateFile>(&s) {
                Ok(f) => Some(f.templates),
                Err(e) => {
                    tracing::warn!("templates file unreadable: {e}");
                    None
                }
            })
            .unwrap_or_default();
        Self {
            templates,
            path: Some(path),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let json = serde_json::to_string_pretty(&TemplateFile {
            version: 1,
            templates: self.templates.clone(),
        })
        .map_err(std::io::Error::other)?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    }

    /// Adds a template under an automatic name and returns that name.
    pub fn add(
        &mut self,
        based_on: Option<&str>,
        video_size: (f32, f32),
        gauges: Vec<Gauge>,
    ) -> String {
        let name = auto_name(based_on, &self.templates);
        self.templates.push(UserTemplate {
            name: name.clone(),
            based_on: based_on.map(|b| root_name(b).to_string()),
            video_size,
            gauges,
            created: chrono::Utc::now(),
        });
        name
    }

    pub fn find(&self, name: &str) -> Option<&UserTemplate> {
        self.templates.iter().find(|t| t.name == name)
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.templates.len();
        self.templates.retain(|t| t.name != name);
        self.templates.len() != before
    }

    /// Renames a template. Fails (returns false) for empty or duplicate names.
    pub fn rename(&mut self, old: &str, new: &str) -> bool {
        let new = new.trim();
        if new.is_empty()
            || self
                .templates
                .iter()
                .any(|t| t.name != old && t.name.eq_ignore_ascii_case(new))
        {
            return false;
        }
        match self.templates.iter_mut().find(|t| t.name == old) {
            Some(t) => {
                t.name = new.to_string();
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gauges::library::{PresetId, make_preset};
    use crate::telemetry::UnitSystem;

    fn tpl(name: &str) -> UserTemplate {
        UserTemplate {
            name: name.into(),
            based_on: None,
            video_size: (1920.0, 1080.0),
            gauges: Vec::new(),
            created: chrono::Utc::now(),
        }
    }

    #[test]
    fn automatic_names() {
        let mut existing = Vec::new();
        assert_eq!(auto_name(Some("Cycling"), &existing), "Cycling (Custom)");
        existing.push(tpl("Cycling (Custom)"));
        assert_eq!(auto_name(Some("Cycling"), &existing), "Cycling (Custom 2)");
        // Saving from a layout that came from a custom template keeps the root name.
        assert_eq!(
            auto_name(Some("Cycling (Custom)"), &existing),
            "Cycling (Custom 2)"
        );
        assert_eq!(auto_name(None, &existing), "Custom Template");
        existing.push(tpl("Custom Template"));
        assert_eq!(auto_name(None, &existing), "Custom Template 2");
        assert_eq!(
            auto_name(Some("Custom Template"), &existing),
            "Custom Template 2"
        );
        assert_eq!(root_name("Running (Custom 12)"), "Running");
        assert_eq!(root_name("My (Custom) layout"), "My (Custom) layout");
        assert_eq!(root_name("Sailing"), "Sailing");
    }

    #[test]
    fn store_roundtrip_rename_remove() {
        let dir = std::env::temp_dir().join(format!("telemetryvibe-tpl-{}", std::process::id()));
        let path = TemplateStore::file_path(&dir);
        let mut store = TemplateStore::load(path.clone());
        assert!(store.templates.is_empty());
        let g = make_preset(
            PresetId::DigitalHr,
            GaugeId(7),
            (1920.0, 1080.0),
            None,
            UnitSystem::Metric,
        );
        let name = store.add(Some("Running"), (1920.0, 1080.0), vec![g.clone()]);
        assert_eq!(name, "Running (Custom)");
        store.save().unwrap();
        let mut back = TemplateStore::load(path.clone());
        assert_eq!(back.templates.len(), 1);
        assert_eq!(back.templates[0].gauges[0], g);
        assert_eq!(back.templates[0].based_on.as_deref(), Some("Running"));
        assert!(back.rename("Running (Custom)", "Trail run"));
        back.add(Some("Running"), (1920.0, 1080.0), Vec::new());
        assert!(!back.rename("Trail run", "running (custom)"), "duplicate");
        assert!(!back.rename("Trail run", "  "), "empty");
        assert!(back.remove("Trail run"));
        assert!(back.find("Trail run").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn instantiate_scales_and_keeps_edges() {
        let mk = |x, y| {
            let mut g = make_preset(
                PresetId::DigitalSpeed,
                GaugeId(1),
                (1920.0, 1080.0),
                None,
                UnitSystem::Metric,
            );
            g.placement.x = x;
            g.placement.y = y;
            g.placement.w = 200.0;
            g.placement.h = 100.0;
            g.group = Some(4);
            g
        };
        let t = UserTemplate {
            gauges: vec![mk(40.0, 940.0), mk(1680.0, 40.0), mk(860.0, 490.0)],
            ..tpl("t")
        };
        let mut id = 100;
        let mut gid = 9;
        let out = t.instantiate(
            (3840.0, 2160.0),
            &mut || {
                id += 1;
                GaugeId(id)
            },
            &mut || {
                gid += 1;
                gid
            },
        );
        // Bottom-left: margins double with the 2× height.
        assert_eq!(
            (out[0].placement.x, out[0].placement.y),
            (80.0, 2160.0 - 80.0 - 200.0)
        );
        assert_eq!((out[0].placement.w, out[0].placement.h), (400.0, 200.0));
        // Top-right keeps its right margin.
        assert_eq!(out[1].placement.x, 3840.0 - 80.0 - 400.0);
        // Centered stays centered.
        assert_eq!(out[2].placement.x, 1920.0 - 200.0);
        // Fresh ids, shared group remapped to one new id.
        assert_eq!(
            out.iter().map(|g| g.id.0).collect::<Vec<_>>(),
            vec![101, 102, 103]
        );
        assert!(out.iter().all(|g| g.group == Some(10)));

        // Narrower (portrait) target: the right-anchored gauge stays inside the frame.
        let out = t.instantiate((1080.0, 1920.0), &mut || GaugeId(1), &mut || 1);
        let p = out[1].placement;
        assert!(p.x >= 0.0 && p.x + p.w <= 1080.0 + 0.5, "{p:?}");
    }
}
