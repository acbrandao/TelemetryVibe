//! Centralized application state: the project document plus loaded media and selection.

use std::path::PathBuf;
use std::sync::Arc;

use super::commands::{Command, UndoStack};
use crate::gauges::RenderCtx;
use crate::gauges::model::{Gauge, GaugeId};
use crate::project::Project;
use crate::project::autosave::Autosaver;
use crate::telemetry::Track;
use crate::video::VideoInfo;

pub const DEFAULT_CANVAS: (f32, f32) = (1920.0, 1080.0);

pub struct AppState {
    pub project: Project,
    pub project_path: Option<PathBuf>,
    pub dirty: bool,
    /// Incremented on every document change; invalidates cached gauge rasters.
    pub revision: u64,
    pub video: Option<VideoInfo>,
    pub track: Option<Arc<Track>>,
    pub selection: Vec<GaugeId>,
    pub undo: UndoStack,
    /// Current video time in seconds.
    pub playhead: f64,
    pub autosaver: Autosaver,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            project: Project::default(),
            project_path: None,
            dirty: false,
            revision: 1,
            video: None,
            track: None,
            selection: Vec::new(),
            undo: UndoStack::default(),
            playhead: 0.0,
            autosaver: Autosaver::default(),
        }
    }
}

impl AppState {
    /// Timeline duration: the video, else the telemetry, else one minute.
    pub fn duration(&self) -> f64 {
        if let Some(v) = &self.video {
            return v.duration.max(0.04);
        }
        self.track
            .as_ref()
            .map(|t| t.duration())
            .filter(|d| *d > 0.0)
            .unwrap_or(60.0)
    }

    pub fn fps(&self) -> f64 {
        self.video.as_ref().map(|v| v.fps).unwrap_or(30.0)
    }

    /// Canvas size in video pixels.
    pub fn video_size(&self) -> (f32, f32) {
        self.video
            .as_ref()
            .map(|v| (v.width as f32, v.height as f32))
            .unwrap_or(DEFAULT_CANVAS)
    }

    pub fn render_ctx(&self) -> RenderCtx<'_> {
        RenderCtx {
            track: self.track.as_deref(),
            sync: &self.project.sync,
            video_t: self.playhead,
            units: self.project.units,
        }
    }

    pub fn gps_time(&self) -> f64 {
        self.project.sync.video_to_gps(self.playhead)
    }

    pub fn touch(&mut self) {
        self.revision += 1;
        self.dirty = true;
    }

    /// Executes an undoable command.
    pub fn execute(&mut self, cmd: Command) {
        self.undo.record(&self.project, &cmd);
        let major = cmd.is_major();
        if let Command::DeleteGauges(ids) = &cmd {
            self.selection.retain(|id| !ids.contains(id));
        }
        cmd.apply(&mut self.project);
        self.autosaver.mark_dirty(major);
        self.touch();
    }

    fn restore(&mut self, snapshot: Project) {
        // Media references and view settings are not part of the undo history.
        let keep_video = self.project.video.clone();
        let keep_tel = self.project.telemetry.clone();
        let keep_timeline = self.project.timeline.clone();
        let keep_preview = self.project.preview.clone();
        self.project = snapshot;
        self.project.video = keep_video;
        self.project.telemetry = keep_tel;
        self.project.timeline = keep_timeline;
        self.project.preview = keep_preview;
        let ids: Vec<GaugeId> = self.project.gauges.iter().map(|g| g.id).collect();
        self.selection.retain(|id| ids.contains(id));
        self.autosaver.mark_dirty(true);
        self.touch();
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.undo(&self.project) {
            Some(p) => {
                self.restore(p);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.undo.redo(&self.project) {
            Some(p) => {
                self.restore(p);
                true
            }
            None => false,
        }
    }

    pub fn selected_gauges(&self) -> Vec<&Gauge> {
        self.project
            .gauges
            .iter()
            .filter(|g| self.selection.contains(&g.id))
            .collect()
    }

    pub fn single_selection(&self) -> Option<&Gauge> {
        match self.selection.as_slice() {
            [id] => self.project.gauge(*id),
            _ => None,
        }
    }

    /// Selects a gauge (expanding to its group). `toggle` adds/removes instead of replacing.
    pub fn select(&mut self, id: GaugeId, toggle: bool) {
        let group = self.project.gauge(id).and_then(|g| g.group);
        let members: Vec<GaugeId> = match group {
            Some(gr) => self
                .project
                .gauges
                .iter()
                .filter(|g| g.group == Some(gr))
                .map(|g| g.id)
                .collect(),
            None => vec![id],
        };
        if toggle {
            if self.selection.contains(&id) {
                self.selection.retain(|s| !members.contains(s));
            } else {
                for m in members {
                    if !self.selection.contains(&m) {
                        self.selection.push(m);
                    }
                }
            }
        } else {
            self.selection = members;
        }
    }

    /// Duplicates the selected gauges with an offset; returns the new ids.
    pub fn duplicate_selection(&mut self) -> Vec<GaugeId> {
        let originals: Vec<Gauge> = self.selected_gauges().into_iter().cloned().collect();
        if originals.is_empty() {
            return Vec::new();
        }
        let mut copies = Vec::new();
        for mut g in originals {
            g.id = self.project.alloc_gauge_id();
            g.placement.x += 24.0;
            g.placement.y += 24.0;
            g.locked = false;
            g.group = None;
            g.name = format!("{} copy", g.name);
            copies.push(g);
        }
        let ids: Vec<GaugeId> = copies.iter().map(|g| g.id).collect();
        self.execute(Command::AddGauges(copies));
        self.selection = ids.clone();
        ids
    }

    pub fn delete_selection(&mut self) {
        let ids: Vec<GaugeId> = self
            .selected_gauges()
            .into_iter()
            .filter(|g| !g.locked)
            .map(|g| g.id)
            .collect();
        if !ids.is_empty() {
            self.execute(Command::DeleteGauges(ids));
        }
    }

    /// Moves unlocked selected gauges by (dx, dy) video pixels.
    pub fn nudge_selection(&mut self, dx: f32, dy: f32) {
        let moved: Vec<Gauge> = self
            .selected_gauges()
            .into_iter()
            .filter(|g| !g.locked)
            .cloned()
            .map(|mut g| {
                g.placement.x += dx;
                g.placement.y += dy;
                g
            })
            .collect();
        if !moved.is_empty() {
            self.execute(Command::UpdateGauges {
                gauges: moved,
                merge: "nudge",
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gauges::library::{PresetId, make_preset};
    use crate::telemetry::UnitSystem;

    fn state_with(n: usize) -> AppState {
        let mut s = AppState::default();
        let mut gs = Vec::new();
        for _ in 0..n {
            let id = s.project.alloc_gauge_id();
            gs.push(make_preset(
                PresetId::DigitalSpeed,
                id,
                (1920.0, 1080.0),
                None,
                UnitSystem::Metric,
            ));
        }
        s.execute(Command::AddGauges(gs));
        s
    }

    #[test]
    fn undo_redo_add_delete() {
        let mut s = state_with(2);
        assert_eq!(s.project.gauges.len(), 2);
        let id = s.project.gauges[0].id;
        s.execute(Command::DeleteGauges(vec![id]));
        assert_eq!(s.project.gauges.len(), 1);
        assert!(s.undo());
        assert_eq!(s.project.gauges.len(), 2);
        assert!(s.redo());
        assert_eq!(s.project.gauges.len(), 1);
        assert!(s.undo());
        assert!(s.undo());
        assert!(s.project.gauges.is_empty());
        assert!(!s.undo());
    }

    #[test]
    fn continuous_edits_merge() {
        let mut s = state_with(1);
        s.undo.break_merge();
        for i in 0..10 {
            let mut g = s.project.gauges[0].clone();
            g.placement.x = i as f32 * 10.0;
            s.execute(Command::UpdateGauges {
                gauges: vec![g],
                merge: "drag",
            });
        }
        assert_eq!(s.project.gauges[0].placement.x, 90.0);
        // One undo returns to the pre-drag position.
        assert!(s.undo());
        assert_ne!(s.project.gauges[0].placement.x, 90.0);
        assert_eq!(s.project.gauges.len(), 1);
    }

    #[test]
    fn offset_and_properties_undo() {
        let mut s = state_with(1);
        s.execute(Command::SetOffset(3.25));
        assert_eq!(s.project.sync.offset, 3.25);
        s.undo.break_merge();
        let mut g = s.project.gauges[0].clone();
        g.opacity = 0.5;
        s.execute(Command::UpdateGauge(g));
        assert!(s.undo());
        assert_eq!(s.project.gauges[0].opacity, 1.0);
        assert!(s.undo());
        assert_eq!(s.project.sync.offset, 0.0);
    }

    #[test]
    fn duplicate_and_groups() {
        let mut s = state_with(2);
        let (a, b) = (s.project.gauges[0].id, s.project.gauges[1].id);
        for g in &mut s.project.gauges {
            g.group = Some(7);
        }
        s.select(a, false);
        assert_eq!(s.selection.len(), 2, "group selects all members");
        let new = s.duplicate_selection();
        assert_eq!(new.len(), 2);
        assert_eq!(s.project.gauges.len(), 4);
        assert!(!new.contains(&a) && !new.contains(&b));
        s.delete_selection();
        assert_eq!(s.project.gauges.len(), 2);
    }

    #[test]
    fn locked_gauges_are_protected() {
        let mut s = state_with(1);
        let id = s.project.gauges[0].id;
        s.project.gauges[0].locked = true;
        s.select(id, false);
        s.delete_selection();
        s.nudge_selection(10.0, 0.0);
        assert_eq!(s.project.gauges.len(), 1);
        assert_eq!(
            s.project.gauges[0].placement.x,
            make_preset(
                PresetId::DigitalSpeed,
                id,
                (1920.0, 1080.0),
                None,
                UnitSystem::Metric
            )
            .placement
            .x
        );
    }
}
