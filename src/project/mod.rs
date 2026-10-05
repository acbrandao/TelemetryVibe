//! Project model, JSON serialization, autosave and recovery.

pub mod autosave;
pub mod model;
pub mod templates;

pub use model::{MediaRef, Project, ProjectError};
