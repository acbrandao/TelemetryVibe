//! Undoable editing commands.
//!
//! All document changes go through [`Command`]s executed by [`super::state::AppState::execute`]:
//!
//! ```text
//! UI ──Command──► AppState ──► Project ──(revision++)──► renderers
//! ```
//!
//! Undo uses document snapshots (the project is small: gauge definitions and settings), and
//! continuous edits (dragging, sliders) are coalesced into a single undo step via merge keys.

use std::time::{Duration, Instant};

use crate::gauges::model::{Gauge, GaugeId};
use crate::project::Project;
use crate::project::model::PreviewSettings;
use crate::telemetry::UnitSystem;
use crate::video::encoder::{RenderSettings, TimeRange};

// Commands are short-lived; boxing the gauge payloads would only add noise.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum Command {
    AddGauges(Vec<Gauge>),
    DeleteGauges(Vec<GaugeId>),
    /// Replace one gauge definition (inspector edits).
    UpdateGauge(Gauge),
    /// Replace several gauges (drag, align); `merge` groups continuous edits.
    UpdateGauges {
        gauges: Vec<Gauge>,
        merge: &'static str,
    },
    /// Replace the full gauge list (templates).
    ReplaceGauges(Vec<Gauge>),
    /// Remove every gauge.
    ClearGauges,
    /// Move a gauge in the stacking order (positive = towards front).
    Reorder {
        id: GaugeId,
        delta: i32,
    },
    SetOffset(f64),
    SetSyncMarks {
        video: Option<f64>,
        gps: Option<f64>,
    },
    SetUnits(UnitSystem),
    SetRender(RenderSettings),
    SetPreview(PreviewSettings),
    /// Timeline in/out points for rendering (`None` = whole video).
    SetTrim(Option<TimeRange>),
}

impl Command {
    pub fn label(&self) -> &'static str {
        match self {
            Command::AddGauges(_) => "Add gauge",
            Command::DeleteGauges(_) => "Delete gauge",
            Command::UpdateGauge(_) => "Change gauge",
            Command::UpdateGauges { .. } => "Edit gauges",
            Command::ReplaceGauges(_) => "Apply template",
            Command::ClearGauges => "Clear all gauges",
            Command::Reorder { .. } => "Reorder",
            Command::SetOffset(_) => "Change sync offset",
            Command::SetSyncMarks { .. } => "Sync marks",
            Command::SetUnits(_) => "Change units",
            Command::SetRender(_) => "Render settings",
            Command::SetPreview(_) => "Preview settings",
            Command::SetTrim(_) => "Trim",
        }
    }

    fn merge_key(&self) -> Option<String> {
        match self {
            Command::UpdateGauge(g) => Some(format!("gauge:{}", g.id.0)),
            Command::UpdateGauges { merge, .. } => Some((*merge).to_string()),
            Command::SetOffset(_) => Some("offset".into()),
            Command::SetRender(_) => Some("render".into()),
            Command::SetPreview(_) => Some("preview".into()),
            Command::SetTrim(_) => Some("trim".into()),
            _ => None,
        }
    }

    /// Major changes trigger a prompt autosave.
    pub fn is_major(&self) -> bool {
        matches!(
            self,
            Command::AddGauges(_)
                | Command::DeleteGauges(_)
                | Command::ReplaceGauges(_)
                | Command::ClearGauges
                | Command::SetOffset(_)
                | Command::SetUnits(_)
        )
    }

    /// Whether the command is recorded in the undo history.
    fn undoable(&self) -> bool {
        !matches!(self, Command::SetPreview(_))
    }

    pub(crate) fn apply(self, p: &mut Project) {
        match self {
            Command::AddGauges(gs) => {
                for g in gs {
                    p.next_gauge_id = p.next_gauge_id.max(g.id.0 + 1);
                    p.gauges.push(g);
                }
            }
            Command::DeleteGauges(ids) => p.gauges.retain(|g| !ids.contains(&g.id)),
            Command::UpdateGauge(g) => {
                if let Some(slot) = p.gauge_mut(g.id) {
                    *slot = g;
                }
            }
            Command::UpdateGauges { gauges, .. } => {
                for g in gauges {
                    if let Some(slot) = p.gauge_mut(g.id) {
                        *slot = g;
                    }
                }
            }
            Command::ReplaceGauges(gs) => {
                for g in &gs {
                    p.next_gauge_id = p.next_gauge_id.max(g.id.0 + 1);
                }
                p.gauges = gs;
            }
            Command::ClearGauges => p.gauges.clear(),
            Command::Reorder { id, delta } => {
                if let Some(i) = p.gauges.iter().position(|g| g.id == id) {
                    let j = (i as i64 + delta as i64).clamp(0, p.gauges.len() as i64 - 1) as usize;
                    let g = p.gauges.remove(i);
                    p.gauges.insert(j, g);
                }
            }
            Command::SetOffset(o) => p.sync.offset = o,
            Command::SetSyncMarks { video, gps } => {
                p.sync.video_mark = video;
                p.sync.gps_mark = gps;
            }
            Command::SetUnits(u) => p.units = u,
            Command::SetRender(r) => p.render = r,
            Command::SetPreview(s) => p.preview = s,
            Command::SetTrim(t) => p.trim = t,
        }
    }
}

struct Entry {
    snapshot: Project,
    label: &'static str,
    merge: Option<String>,
    at: Instant,
}

const MERGE_WINDOW: Duration = Duration::from_millis(900);
const MAX_UNDO: usize = 200;

/// Snapshot-based undo/redo history.
#[derive(Default)]
pub struct UndoStack {
    undo: Vec<Entry>,
    redo: Vec<(Project, &'static str)>,
    break_merge: bool,
}

impl UndoStack {
    /// Records the state *before* applying `cmd`. Returns false if merged into the last step.
    pub(crate) fn record(&mut self, before: &Project, cmd: &Command) -> bool {
        if !cmd.undoable() {
            return false;
        }
        let key = cmd.merge_key();
        let now = Instant::now();
        if !self.break_merge
            && let (Some(k), Some(last)) = (&key, self.undo.last_mut())
            && last.merge.as_ref() == Some(k)
            && now.duration_since(last.at) < MERGE_WINDOW
        {
            last.at = now;
            self.redo.clear();
            return false;
        }
        self.break_merge = false;
        self.undo.push(Entry {
            snapshot: before.clone(),
            label: cmd.label(),
            merge: key,
            at: now,
        });
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
        true
    }

    /// Forces the next command to start a new undo step (e.g. at the start of a drag).
    pub fn break_merge(&mut self) {
        self.break_merge = true;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_label(&self) -> Option<&'static str> {
        self.undo.last().map(|e| e.label)
    }

    pub fn redo_label(&self) -> Option<&'static str> {
        self.redo.last().map(|e| e.1)
    }

    pub(crate) fn undo(&mut self, current: &Project) -> Option<Project> {
        let e = self.undo.pop()?;
        self.redo.push((current.clone(), e.label));
        self.break_merge = true;
        Some(e.snapshot)
    }

    pub(crate) fn redo(&mut self, current: &Project) -> Option<Project> {
        let (p, label) = self.redo.pop()?;
        self.undo.push(Entry {
            snapshot: current.clone(),
            label,
            merge: None,
            at: Instant::now(),
        });
        self.break_merge = true;
        Some(p)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
