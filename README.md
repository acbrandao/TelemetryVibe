# TelemetryVibe

A **vibe-coded** (via Claude) native desktop app (macOS first, Windows-ready) that adds animated GPS telemetry gauges to video.
The workflow is: drop a video and a FIT/GPX/TCX file, let it sync automatically, pick a template,
arrange the gauges on the video, scrub to check, then render.

Written in Rust: egui/eframe on **wgpu** for the UI, **tiny-skia** for vector gauge rendering,
and **[FFmpeg](https://ffmpeg.org/)** (run as a subprocess) for probing, decoding, proxies and final encoding.

FFmpeg reads almost any camera format (GoPro, DJI, iPhone HEVC, and others) and uses the computer's
hardware encoders (VideoToolbox, NVENC, QSV, AMF). It can overlay the gauges and encode the output in a
single pass, so the original audio and quality are preserved. Running it as a separate process means the
app doesn't need to bundle or link codec libraries, and you can switch to any FFmpeg build.

## Binaries

Prebuilt downloads are attached to [GitHub Releases](https://github.com/acbrandao/TelemetryVibe/releases).

| Platform | Architecture | Download | Notes |
|---|---|---|---|
| macOS | Apple Silicon (arm64) | [TelemetryVibe-0.1.0-macos-arm64.zip](https://github.com/acbrandao/TelemetryVibe/releases/download/v0.1.0/TelemetryVibe-0.1.0-macos-arm64.zip) | Ad-hoc signed: right-click → **Open** the first time. Requires FFmpeg (`brew install ffmpeg`). |
| macOS | Intel (x86_64) | Not yet available | Build with `UNIVERSAL=1 scripts/bundle_macos.sh` |
| Windows | x64 / ARM64 | Not yet available | Build with `scripts\build_windows.ps1` |
| Linux | x86_64 | Not yet available | See [platforms.md](platforms.md) |

## Quick start

See [platforms.md](platforms.md) for one-command setup and build scripts for macOS, Windows and Linux.

```bash
brew install ffmpeg                       # or any FFmpeg build; the location is configurable in Settings
cargo run --release                       # launch the app
cargo run --release --example make_sample # optional: generates sample/sample_ride.{fit,mp4}
```

Drop `sample/sample_ride.mp4` and `sample/sample_ride.fit` onto the window. The video's creation time
sits 60 s into the ride, so automatic sync applies an offset of `+01:00.000`. Choose **Cycling** in the
template picker and press **Render Video**.

You can also open a project file directly: `telemetryvibe path/to/project.gpsvideo`.

### Building app bundles

```bash
scripts/bundle_macos.sh                 # dist/TelemetryVibe.app (icon, .gpsvideo document type, ad-hoc signed)
UNIVERSAL=1 scripts/bundle_macos.sh     # arm64 + x86_64 universal binary
FFMPEG_DIR=/path/to/static scripts/bundle_macos.sh   # bundle a static ffmpeg/ffprobe in Resources
```

On Windows, run `scripts\build_windows.ps1` (x64 by default, `-Target aarch64-pc-windows-msvc` for ARM64).

## Using the app

| Area | What it does |
|---|---|
| Toolbar | Import Video / Import GPS, Sync, Template (built-in and saved templates, **Save Current Template**), preview quality (Draft/Half/Full), units, Render |
| Left panel | Video metadata (resolution, fps, duration, codec, audio, size, recording time, proxy status), GPS summary and the channels it contains, layer list (show/hide, lock, select) |
| Canvas | Video with live gauges. Click to select, drag to move, use handles to resize or rotate. Snapping guides appear automatically (hold ⌥/Alt to disable). ⇧ constrains movement, keeps aspect ratio, or snaps rotation to 15°. Right-click for duplicate, lock, hide, ordering and grouping. Drag gauges here from the library. |
| Right panel | **Gauge Library** (templates plus about 50 presets with live preview icons, including a beating-heart heart-rate readout, zone-coded HR/power dials (zone arc, LED ring, peak hold), a digital gradient with slope icon, and a close-up moving map with a speed-colored pointer) and **Inspector** (data source, units, range, position/size/rotation, colors, font, shadow, glow, labels, kind-specific options, color zones, smoothing) |
| Timeline | Transport controls, video/GPS wall-clock readout, offset field with ±1 s / ±0.1 s / ±1 frame nudges, ruler with zoom (⌘/Ctrl + scroll), thumbnail strip, GPS track with a speed sparkline (**drag it to sync**), gauges track. **Trim:** drag the yellow start/end handles (or press I / O at the playhead, ⌥X to clear) to render only that section; handles snap to frames and to the GPS recording's start and end |
| Sync window | 1) automatic from video metadata, with ±1 h timezone fixes; 2) manual offset; 3) event matching: mark a moment in the video, click the same moment on a speed graph of the whole recording (or pick a detected start or stop), then Align |
| Templates | **Save Current Template** stores the gauge layout and settings in `templates.json` in the config folder, named after the template it started from ("Cycling (Custom)", "Cycling (Custom 2)", …). Saved templates appear under **My Templates** in every template menu and adapt to other resolutions and aspect ratios. Right-click one in the library to rename or delete it. |
| Render | Renders the trimmed section (shown as Range) or the whole video. H.264/H.265, resolution, frame rate, quality, Automatic/CPU/Hardware (shows the encoder that will be used), keep audio, size estimate. The queue window shows progress, encoding fps, ETA, pause/resume, cancel and Open Folder. |

Keyboard shortcuts are listed under **Help → Keyboard Shortcuts**. The arrow keys step frames, or nudge
gauges when a gauge is selected (⇧ moves 10×). `,` and `.` always step frames.

## Architecture

```text
src/
  telemetry/  FIT parser (written from scratch), GPX/TCX, columnar Track model, derived channels,
              per-metric interpolation, stateless smoothing, sync, units
  gauges/     Gauge definitions (serde), per-kind builders → vector Scene, library and templates
  render/     tiny-skia rasterizer + glyph-outline text (shared by preview and export),
              overlay compositor, render jobs and queue
  video/      FFmpeg discovery and HW encoder verification, ffprobe, async preview decoder,
              frame cache, thumbnails, proxies, export command builder
  project/    .gpsvideo JSON documents, autosave, crash recovery, recent files, settings
  app/        AppState, undoable Commands (UI → Command → State → Project → renderers), eframe app
  ui/         egui panels: canvas, timeline, inspector, library, sync, render, settings, theme
  platform/   the only OS-specific code: reveal in file manager, app icon
```

Key design decisions:

- **GPS altitude.** FIT files from Garmin devices carry the GPS receiver's altitude in
  `gps_metadata` messages (`enhanced_altitude`), separate from the barometric record altitude. It is
  read as the *GPS Altitude* channel (a developer field named `gps_altitude` also works). Altitude
  presets, templates and the elevation graph default to it when present, and gradient, vertical
  speed and elevation gain/loss are derived from a smoothed version of it. Gauges bound to GPS
  altitude fall back to the regular altitude for files without it.

- **Preview matches export.** Each gauge builds a vector `Scene` from its definition and the telemetry
  at a given instant. The same `draw_scene` rasterizes it at display resolution for the preview (uploaded
  as a texture and drawn as a rotated quad on the GPU) and at output resolution for export. Text is drawn
  as glyph outlines from the bundled Inter variable font, with tabular digits so numbers don't jitter.
- **Single-pass export.** FFmpeg decodes the original once, scales and retimes it, and applies the
  `overlay` filter with an RGBA stream piped over stdin, then encodes once with VideoToolbox, NVENC,
  QSV, AMF, Media Foundation or x264/x265. Only the bounding region covered by gauges is piped. Overlay
  frames are rendered in parallel with rayon while earlier ones stream to FFmpeg. Audio is copied, or
  transcoded to AAC when the codec isn't MP4-compatible.
- **The UI thread never blocks.** Decoding, probing, parsing, thumbnails, proxies and rendering run on
  worker threads. The preview decoder coalesces seeks (latest wins) and decodes a small burst around the
  target so frame stepping is instant. Playback streams into a bounded ring buffer driven by the UI clock.
  Generation counters discard stale frames.
- **Deterministic smoothing.** The 0–100 % smoothing control is a Gaussian low-pass over the interpolated
  signal, evaluated statelessly at time *t*, so scrubbing, playback and export produce identical values.
  Interpolation is linear for most metrics, circular for heading and cubic Hermite for (pre-smoothed)
  altitude. Gaps longer than 60 s are not bridged.
- **Extensible.** New telemetry formats implement `TelemetryParser`. New gauges add a `GaugeKind`
  variant and a builder. Unknown FIT record fields and developer fields are kept as extra channels that
  any gauge can bind to ("Custom" source). Gauges carry a `keyframes` field reserved for animation.

## Tests

```bash
cargo test --release        # 94 unit tests + 2 end-to-end tests
cargo clippy --all-targets  # clean
```

The end-to-end tests generate a FIT file and a test video, auto-sync them, apply a template, render
through FFmpeg, and check the output's size, duration, preserved audio and visible overlay. They also
exercise the decoder's seek and playback paths. They skip themselves if FFmpeg isn't installed.

Developer tools:

- `cargo run --example gauge_sheet -- out.png` renders every preset to a contact sheet.
- `cargo run --example decode_bench -- video.mp4` measures seek latency and decode throughput.
- `cargo run --example fit_inspect -- ride.fit` lists a FIT file's messages, record fields and parsed
  channels, including barometric vs GPS altitude (`FIT_LAG=1` also checks their time alignment).
- `TELEMETRYVIBE_DEV_SCRIPT="wait=3;template=Cycling;playhead=20;shot=/tmp/a.png;quit"` drives the UI and
  saves window screenshots, for visual checks.

Logs are written to the platform data directory (`~/Library/Application Support/net.TelemetryVibe.TelemetryVibe/logs`
on macOS). Open them with **Help → Open Logs**.

## Status and known limitations

The MVP and first-milestone workflow are complete: import → parse → preview → sync → gauges →
drag/resize → render MP4 with audio → save/load. Current limitations:

- **No audio during preview playback.** Audio is preserved in the rendered video.
- **Menus are drawn in the window**, not in the native macOS menu bar.
- **Double-clicking a `.gpsvideo` file in Finder** launches the app but doesn't open the project. eframe
  doesn't expose the macOS open-document event. Opening from the command line, the File menu or drag-and-drop works.
- **Windows** builds from the same code (OS-specific code is isolated in `platform/`), but it hasn't been
  built or tested on Windows yet. There's no installer yet.
- **Keyframe animation** exists only in the data model. Gauges are static over time, as the
  requirements specify for the first version.
- **Subprocess decoding latency.** Scrubbing seeks take about 150 ms for 1080p H.264. 4K/HEVC sources
  should use the proxy, which is generated automatically for sources above 1440p.
- **Parsed telemetry isn't cached on disk.** A large FIT file parses in milliseconds, so this hasn't
  been needed.

Fonts: Inter, © The Inter Project Authors, SIL Open Font License 1.1 (`assets/fonts/OFL-Inter.txt`).
