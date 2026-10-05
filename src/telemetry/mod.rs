//! Telemetry: file parsers, normalized model, interpolation, smoothing and synchronization.
//!
//! New formats are added by implementing [`TelemetryParser`] and registering it in
//! [`parsers`]; nothing else in the application needs to change.

pub mod fit;
pub mod interpolation;
pub mod model;
pub mod smoothing;
pub mod sync;
pub mod units;
pub mod xml_formats;

use std::path::Path;

pub use model::{Metric, TelemetrySample, Track};
pub use sync::SyncSettings;
pub use units::{UnitPref, UnitSystem};

/// User-facing telemetry errors. Technical detail is kept in the source chain for logs.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error(
        "Unable to read GPS file. The file may be corrupted or use an unsupported FIT profile."
    )]
    InvalidFit(#[source] fit::FitError),
    #[error("Unable to read GPS file. The {format} file could not be parsed.")]
    InvalidXml {
        format: &'static str,
        #[source]
        source: xml_formats::XmlError,
    },
    #[error("Unsupported GPS file format. Supported formats: FIT, GPX, TCX.")]
    Unsupported,
    #[error("Unable to open the GPS file.")]
    Io(#[from] std::io::Error),
}

/// A telemetry file format parser module.
pub trait TelemetryParser: Send + Sync {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    /// Content sniffing, used when the extension is missing or wrong.
    fn sniff(&self, bytes: &[u8]) -> bool;
    fn parse(&self, bytes: &[u8], file_name: &str) -> Result<Track, TelemetryError>;
}

struct FitParser;
impl TelemetryParser for FitParser {
    fn name(&self) -> &'static str {
        "Garmin FIT"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["fit"]
    }
    fn sniff(&self, bytes: &[u8]) -> bool {
        fit::sniff(bytes)
    }
    fn parse(&self, bytes: &[u8], file_name: &str) -> Result<Track, TelemetryError> {
        fit::parse(bytes, file_name).map_err(TelemetryError::InvalidFit)
    }
}

struct GpxParser;
impl TelemetryParser for GpxParser {
    fn name(&self) -> &'static str {
        "GPX"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["gpx"]
    }
    fn sniff(&self, bytes: &[u8]) -> bool {
        xml_formats::sniff_gpx(bytes)
    }
    fn parse(&self, bytes: &[u8], file_name: &str) -> Result<Track, TelemetryError> {
        xml_formats::parse_gpx(bytes, file_name).map_err(|source| TelemetryError::InvalidXml {
            format: "GPX",
            source,
        })
    }
}

struct TcxParser;
impl TelemetryParser for TcxParser {
    fn name(&self) -> &'static str {
        "TCX"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["tcx"]
    }
    fn sniff(&self, bytes: &[u8]) -> bool {
        xml_formats::sniff_tcx(bytes)
    }
    fn parse(&self, bytes: &[u8], file_name: &str) -> Result<Track, TelemetryError> {
        xml_formats::parse_tcx(bytes, file_name).map_err(|source| TelemetryError::InvalidXml {
            format: "TCX",
            source,
        })
    }
}

/// All registered parsers.
pub fn parsers() -> Vec<Box<dyn TelemetryParser>> {
    vec![
        Box::new(FitParser),
        Box::new(GpxParser),
        Box::new(TcxParser),
    ]
}

/// All file extensions any parser accepts.
pub fn supported_extensions() -> Vec<&'static str> {
    parsers()
        .iter()
        .flat_map(|p| p.extensions().iter().copied())
        .collect()
}

pub fn is_telemetry_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    supported_extensions().contains(&ext.as_str())
}

/// Parses telemetry from bytes, choosing the parser by extension then by content.
pub fn parse_bytes(bytes: &[u8], file_name: &str) -> Result<Track, TelemetryError> {
    let ext = Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    let all = parsers();
    let parser = all
        .iter()
        .find(|p| p.extensions().contains(&ext.as_str()) && p.sniff(bytes))
        .or_else(|| all.iter().find(|p| p.sniff(bytes)))
        .or_else(|| all.iter().find(|p| p.extensions().contains(&ext.as_str())))
        .ok_or(TelemetryError::Unsupported)?;
    parser.parse(bytes, file_name)
}

/// Loads and parses a telemetry file.
pub fn load_file(path: &Path) -> Result<Track, TelemetryError> {
    let bytes = std::fs::read(path)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "telemetry".into());
    let track = parse_bytes(&bytes, &name)?;
    tracing::info!(
        file = %path.display(),
        samples = track.len(),
        duration = track.duration(),
        metrics = ?track.available_metrics(),
        "telemetry loaded"
    );
    Ok(track)
}

/// Human-readable message for a metric missing from the recording.
pub fn missing_metric_message(m: Metric) -> String {
    format!(
        "This GPS recording does not contain {} data.",
        m.label().to_lowercase()
    )
}
