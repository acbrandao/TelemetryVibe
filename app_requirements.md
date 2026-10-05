# Rust GPS Video Gauge Overlay Application

## Project Overview

Develop a professional desktop application for **macOS initially, with Windows support as a first-class target**, that creates animated GPS telemetry gauge overlays on video.

The application should provide a workflow similar in concept to **GaugeReel.net**, including GPS-data-driven gauges, video synchronization, live gauge positioning, timeline-based editing, previewing, and final video rendering.

Reference feature set:

* https://gaugereel.net/features

The application takes:

1. A base video file
2. A GPS/telemetry data file such as Garmin `.FIT`
3. Optional additional GPS formats
4. User-selected gauge designs and layouts

It then synchronizes the telemetry with the video, displays animated gauges over the video, allows the user to position and resize the gauges visually, and finally renders a new video with the telemetry graphics composited into the video.

The application should feel like a **simple, modern NLE video editor**, but specifically optimized for GPS/sports telemetry overlays rather than general-purpose video editing.

---

# 1. Technology Requirements

## Primary Language

Use:

**Rust**

The architecture should be designed so that the majority of application logic, data processing, telemetry processing, rendering coordination, and file handling are implemented in Rust.

Avoid unnecessary JavaScript-heavy architecture.

---

# 2. Cross-Platform Requirements

Initial target:

* macOS
* Apple Silicon
* Intel macOS if practical

Second target:

* Windows 10/11
* x64
* ARM64 where practical

The codebase must avoid platform-specific APIs wherever possible.

Use cross-platform Rust libraries.

Do not create separate Mac and Windows implementations unless absolutely necessary.

---

# 3. Native UI Framework

Evaluate and select the best mature Rust-native UI framework for this application.

Preferred candidates:

* Slint
* egui
* iced

The UI must provide:

* Hardware-accelerated rendering
* Smooth animation
* High-DPI support
* Retina support
* Resizable windows
* Native file dialogs
* Drag-and-drop
* Keyboard shortcuts
* Dark/light themes
* macOS and Windows compatibility

Prioritize **performance and video interaction** over visual gimmicks.

The application should feel like a native professional desktop application.

If Slint provides the best combination of performance, maintainability and native desktop behavior, prefer Slint.

Use:

**wgpu**

for GPU-accelerated rendering wherever appropriate.

---

# 4. Video Processing

Use **FFmpeg extensively**.

Do not implement a custom video codec or video encoding pipeline.

FFmpeg should handle:

* Video decoding where appropriate
* Video encoding
* Frame extraction
* Seeking
* Scaling
* Pixel format conversion
* Audio preservation
* Video compositing
* Final rendering
* Hardware-accelerated encoding when available

Support common video formats:

* MP4
* MOV
* MKV
* M4V

Common codecs:

* H.264
* H.265/HEVC
* ProRes where supported

Preserve the original audio track unless the user chooses otherwise.

---

# 5. Hardware Acceleration

Detect and use hardware acceleration whenever available.

macOS:

* VideoToolbox
* Apple Silicon hardware encoding/decoding where supported

Windows:

* NVENC
* Intel Quick Sync
* AMD hardware acceleration
* Media Foundation where useful

FFmpeg should be configured dynamically according to available hardware.

The application should have a rendering preference:

### Automatic

Select the best available hardware encoder.

### CPU

Force software encoding.

### Hardware

Use the detected hardware encoder.

Display the selected encoder in rendering settings.

---

# 6. Core Workflow

The primary workflow should be extremely simple:

## Step 1 — Import Video

User imports a video.

Display:

* Filename
* Resolution
* Frame rate
* Duration
* Codec
* Audio presence
* File size

Generate a low-resolution preview/proxy if necessary.

---

## Step 2 — Import GPS Data

Support:

### Primary

Garmin `.FIT`

### Additional formats

Design the telemetry system so additional formats can be added through parser modules.

---

# 7. FIT File Processing

Implement a robust FIT parser.

Extract as much useful telemetry as possible.

Important fields include:

### Position

* Latitude
* Longitude
* GPS position
* Track
* Heading/course

### Speed

* Speed
* Maximum speed
* Average speed

### Elevation

* Altitude
* Elevation gain
* Elevation loss

### Distance

* Recorded distance
* Lap distance
* Total distance

### Heart Rate

* Heart rate
* Average heart rate
* Maximum heart rate

### Cycling

* Cadence
* Power
* Average power
* Maximum power

### Temperature

* Temperature

### Time

* Date
* Time
* Timestamp
* Recording start time

Also preserve any other telemetry fields found in the FIT file so the architecture can support additional metrics later.

---

# 8. Telemetry Data Model

Create a normalized internal telemetry structure.

Example conceptual structure:

TelemetrySample:

* timestamp
* latitude
* longitude
* altitude
* speed
* distance
* heart_rate
* cadence
* power
* temperature
* heading

The telemetry engine must support:

* Missing values
* Different sample frequencies
* Interpolation
* Smoothing
* Timestamp normalization
* Unit conversion

Do not assume every GPS file contains every field.

---

# 9. Unit System

Support:

### Metric

* km/h
* meters
* kilometers
* °C

### Imperial

* mph
* feet
* miles
* °F

Allow the user to switch units globally.

Individual gauges should inherit global units unless explicitly configured otherwise.

---

# 10. Video/GPS Synchronization

This is one of the most important features.

The application must allow the user to synchronize the GPS recording with the video.

Provide sliders and visual method to synchronize  synchronizgps data with video time , make it intuitive..



## Method B — Manual time offset

Provide:

**GPS Offset**

Example:

`+00:03.250`

or

`-00:12.800`

Allow millisecond-level precision.

---

## Method C — Visual/event synchronization

Allow the user to select a recognizable event in the video and associate it with the corresponding GPS timestamp.

Examples:

* Starting movement
* Crossing a road
* Turning
* Accelerating
* Stopping

---

# 11. Synchronization Timeline

Display a visual relationship between:

**Video Timeline**

and

**GPS Timeline**

The user should be able to drag the GPS timeline relative to the video.

When scrubbing the video, telemetry values should update immediately.

Example:

`Video 00:04:32.240`

corresponds to:

`GPS 14:31:27.582`

Show the current synchronization offset.

---

# 12. Telemetry Interpolation

Telemetry data will often have a lower sampling frequency than video.

Implement interpolation so gauges remain smooth.

For example:

GPS:

10 Hz

Video:

30/60/120 FPS

Calculate interpolated telemetry values between GPS samples.

Use appropriate interpolation for each metric.

Examples:

* Speed → linear interpolation
* Altitude → smoothed interpolation
* Heart rate → interpolation/filtering
* Cadence → interpolation
* Power → interpolation
* Heading → circular interpolation

Avoid visible jumps in gauge animations.

---

# 13. Gauge System

Create a modular gauge system.

Every gauge should be an independent reusable component. represented by vectors

Each gauge should define:

* Name
* Type
* Data source
* Minimum
* Maximum
* Units
* Position
* Size
* Rotation
* Opacity
* Color
* Font
* Background
* Animation behavior

Gauge components should receive a telemetry sample and render themselves.

---

# 14. Gauge Categories

Support several major visual categories.

## A. Analog Dials & Speedometers

Traditional circular gauges with animated needles.

Examples:

* Speedometer
* Heart rate
* Cadence
* Power
* RPM-style gauge

Features:

* Circular dial
* Tick marks
* Numeric labels
* Animated needle
* Center hub
* Colored warning zones
* Configurable min/max
* Configurable units

Visual style options:

* Automotive
* Motorsport
* Aviation
* Minimal
* Modern digital
* Sport

---

# 15. Digital Readouts

Large numerical telemetry displays.

Examples:

**32.4 MPH**

**148 BPM**

**285 W**

**1,245 FT**

Options:

* Large number
* Small unit
* Label
* Decimal precision
* Prefix/suffix
* Background
* Border
* Rounded rectangle
* Transparent

---

# 16. Tape / Linear Gauges

Create modern horizontal and vertical scrolling gauges.

Examples:

### Altitude tape

Vertical scrolling altitude scale.

### Speed tape

Vertical or horizontal scrolling speed.

### Heading tape

Compass heading.

### Gradient tape

Percentage grade.

The current value remains centered while the scale moves behind it.

Support:

* Major tick marks
* Minor tick marks
* Numeric labels
* Current-value marker
* Direction indicators

These should resemble modern aircraft HUDs and professional sports/video telemetry systems.

---

# 17. Progress Bars

Create linear progress-style gauges.

Examples:

* Heart rate intensity
* Power
* Speed
* Gradient
* Cadence
* Distance progress
* Route progress

Support:

* Horizontal
* Vertical
* Rounded
* Thin
* Thick
* Segmented

Allow configurable thresholds and zones.

---

# 18. Dynamic Effort / Zone Gauges

Create color/zone-based telemetry gauges.

Examples:

Heart Rate Zones:

* Zone 1
* Zone 2
* Zone 3
* Zone 4
* Zone 5

Power Zones:

* Recovery
* Endurance
* Tempo
* Threshold
* VO2
* Anaerobic

The gauge should dynamically indicate the current zone.

Allow users to configure their own thresholds.

---

# 19. Historical Graphs

Create small real-time historical charts.

Examples:

* Speed over previous 30 seconds
* Elevation
* Heart rate
* Power
* Cadence
* Temperature

The graph should scroll as the video plays.

Allow configuration of:

* Time window
* Line thickness
* Scale
* Min/max
* Labels
* Grid
* Fill
* Transparency

Use GPU rendering where possible.

---

# 20. GPS Map / Track Gauge

Create a map/route overlay.

The map can display:

* Current location
* Complete route
* Current position
* Direction
* Heading
* Travel trail

Support two primary visual modes.

### Minimal Vector Map

Clean dark/light vector-style route.


---

# 21. Route Visualization

Provide options:

### Full route

Display the entire recorded route.

### Progressive route

Only display the portion already traveled.

### Trail

Show the previous X seconds/minutes.

### Current position

Show a moving marker.

### Direction

Use an arrow showing direction of travel.

Allow:

* Route color
* Route thickness
* Current position marker
* Marker size
* Glow
* Opacity

---

# 22. Gauge Editor

The gauge editor is central to the application.

Provide a visual canvas showing the actual video.

Users should be able to:

* Drag gauges
* Resize gauges
* Rotate gauges
* Duplicate gauges
* Delete gauges
* Lock gauges
* Hide gauges
* Align gauges
* Snap gauges
* Group gauges

Use bounding boxes and handles.

---

# 23. Direct Manipulation

When the user selects a gauge:

Display:

* Bounding box
* Resize handles
* Rotation handle
* Position coordinates

Support:

* Drag
* Resize
* Shift-drag
* Keyboard arrow movement
* Shift + arrow for larger movement

Provide alignment guides.

---

# 24. Gauge Inspector

Provide a right-side properties panel.

Example:

### Gauge

`Speedometer`

### Data

`Speed`

### Position

X: 72

Y: 80

### Size

Width: 260

Height: 260

### Range

0–60 MPH

### Appearance

Opacity: 90%

Needle: White

Accent: Red

Background: 65%

### Animation

Smoothing: 70%

---

# 25. Timeline

The application should have a simplified NLE-style timeline.

Layout:

```text
------------------------------------------------
                 VIDEO PREVIEW
------------------------------------------------

00:00       00:30       01:00       01:30
|-----------|-----------|-----------|

VIDEO      █████████████████████████████

GPS        █████████████████████████████

GAUGES     █████████████████████████████

------------------------------------------------
```

The timeline should support:

* Play
* Pause
* Stop
* Scrub
* Jump to beginning
* Jump to end
* Frame stepping
* Zoom
* Timecode
* Current position indicator

---

# 26. Gauge Timeline Animation

Initially, gauges should remain static for the entire video.

However, design the architecture so gauge properties can later be keyframed.

Potential future keyframes:

* Position
* Size
* Opacity
* Rotation
* Visibility

Do not require full keyframe editing in the initial version, but keep the architecture extensible.

---

# 27. Video Preview

The preview window must be highly optimized.

Prioritize:

**smooth playback and scrubbing.**

Avoid re-rendering the entire video every time the user moves the playhead.

Use:

* Hardware video decoding when available
* Frame caching
* Preview resolution scaling
* GPU compositing
* Asynchronous decoding
* Background frame preparation
* Ring buffers
* Proxy video when needed

---

# 28. Proxy Video System

For large 4K/5K/6K/8K videos, automatically offer proxy generation.

Example:

Original:

`4K 60 FPS`

Proxy:

`1080p 30 FPS`

Preview uses proxy.

Final render uses original.

The user should be able to toggle:

**Preview Quality**

* Draft
* Half
* Full

---

# 29. Scrubbing Performance

Scrubbing should feel immediate.

When dragging the timeline:

1. Decode the nearest keyframe
2. Decode forward to requested frame
3. Display preview immediately
4. Cache nearby frames

Do not block the UI thread.

Use worker threads for decoding and processing.

---

# 30. Rendering Architecture

Separate:

### Preview Renderer

GPU-based interactive rendering.

### Final Renderer

FFmpeg-based final rendering.

The preview renderer and final renderer should use the same gauge definitions so the output matches the preview as closely as possible.

---

# 31. Final Rendering

When the user clicks:

**Render Video**

Create the final output using FFmpeg.

Pipeline concept:

```text
Original Video
      +
Telemetry Timeline
      +
Gauge Definitions
      ↓
GPU/CPU Overlay Renderer
      ↓
FFmpeg Composite
      ↓
Final Video
```

Use FFmpeg filters and hardware acceleration where practical.

Do not unnecessarily encode/decode the video multiple times.

---

# 32. Rendering Options

Provide:

### Output format

MP4

### Codec

* H.264
* H.265

### Resolution

* Original
* 4K
* 1440p
* 1080p
* 720p

### Frame rate

* Original
* 60
* 30
* 24

### Quality

* Draft
* High
* Maximum

### Hardware acceleration

Automatic / CPU / Hardware

---

# 33. Render Queue

Create a render queue.

Display:

* Filename
* Duration
* Resolution
* Codec
* Estimated size
* Progress
* Encoding FPS
* ETA
* Current status

Allow:

* Start
* Pause if supported
* Cancel
* Open output folder

---

# 34. Project File

Create a native project format.

Example:

`.gpsvideo`

The project should store:

* Video reference
* GPS file reference
* Synchronization offset
* Gauge definitions
* Gauge positions
* Gauge sizes
* Gauge settings
* Units
* Rendering settings
* Timeline settings

Do not embed the original video into the project.

Use a portable JSON or binary project structure.

Prefer JSON initially for easy debugging and future compatibility.

---

# 35. Project Recovery

Automatically save project state.

Provide:

* Autosave
* Crash recovery
* Recent projects

Autosave interval:

5 minutes

Also save immediately after major changes when practical.

---

# 36. UI Layout

Use a professional but simple layout.

Suggested:

```text
┌─────────────────────────────────────────────────────────────┐
│ File   Edit   View   Project   Render                       │
├─────────────────────────────────────────────────────────────┤
│ Import Video   Import GPS   Sync   Preview   Render         │
├───────────────┬───────────────────────────┬─────────────────┤
│               │                           │                 │
│  MEDIA        │                           │  GAUGE          │
│               │       VIDEO PREVIEW       │  LIBRARY        │
│  Video        │                           │                 │
│  GPS          │                           │  Speed          │
│               │                           │  Altitude       │
│               │                           │  Heart Rate     │
│               │                           │  Cadence        │
│               │                           │  Power          │
│               │                           │  Map            │
│               │                           │                 │
├───────────────┴───────────────────────────┴─────────────────┤
│                        TIMELINE                              │
│                                                             │
│ Video   ████████████████████████████████████████████████    │
│ GPS     ████████████████████████████████████████████████    │
│ Gauges  ████████████████████████████████████████████████    │
└─────────────────────────────────────────────────────────────┘
```

---

# 37. Gauge Library

Create a left/right gauge library containing predefined gauges.

Initial gauges:

### Speed

* Analog speedometer
* Digital speed
* Speed tape
* Speed bar

### Altitude

* Digital altitude
* Altitude tape
* Elevation graph

### Heart Rate

* Analog HR
* Digital HR
* HR bar
* HR zone
* HR graph

### Cadence

* Digital cadence
* Circular cadence
* Cadence bar

### Power

* Digital power
* Power gauge
* Power zone
* Power graph

### Temperature

* Digital temperature
* Temperature bar

### Distance

* Digital distance
* Distance progress

### Route

* Full route map
* Moving route
* Trail map

### Time

* Date
* Time
* Elapsed time
* Recording time

---

# 38. Gauge Templates

Allow users to start with templates.

Examples:

## Cycling

Speed + Cadence + Power + Heart Rate + Map

## Running

Pace + Heart Rate + Distance + Route

## Sailing

Speed + Direction + Distance + Route + Wind-related telemetry if available

## Hiking

Altitude + Distance + Heart Rate + Route

## Motorsport

Speed + RPM + Track + Lap Time

Templates should simply instantiate multiple gauges.

---

# 39. Gauge Customization

Every gauge should support a consistent customization system.

Properties may include:

* Font
* Font size
* Font weight
* Primary color
* Secondary color
* Background
* Background opacity
* Border
* Border width
* Corner radius
* Shadow
* Glow
* Units
* Decimal precision
* Labels
* Tick marks
* Animation smoothing

---

# 40. Gauge Animation

Telemetry should animate smoothly.

Do not allow gauges to visually jump between GPS samples.

Implement:

* Interpolation
* Exponential smoothing
* Low-pass filtering

Provide:

**Smoothing**

0–100%

A value of 0 means raw data.

Higher values create smoother visual motion.

---

# 41. Color Zones

Allow gauges to define zones.

Example speed:

```text
0–20    Normal
20–35   Fast
35–50   Very Fast
50+     Maximum
```

Zones can affect:

* Gauge arc
* Needle
* Number
* Bar
* Background

---

# 42. Text / Branding Overlay

Allow basic text overlays.

Examples:

* Rider name
* Event name
* Location
* Date
* Custom text

Properties:

* Font
* Size
* Color
* Opacity
* Position

Also support an image/logo overlay.

---

# 43. Drag-and-Drop

Support:

* Drag video into application
* Drag GPS file into application
* Drag gauge into preview
* Drag gauge to reposition
* Drag gauge library items onto canvas

---

# 44. File Handling

Support very large files.

Do not load entire videos into memory.

Use streaming and random access.

Handle:

* Long recordings
* 4K video
* 60/120 FPS
* Large FIT files

---

# 45. Architecture

Use a clean modular Rust architecture.

Suggested structure:

```text
src/

app/
    application
    commands
    state

ui/
    windows
    panels
    timeline
    inspector
    gauge_library

video/
    decoder
    encoder
    ffmpeg
    proxy
    frame_cache

telemetry/
    fit
    gpx
    tcx
    csv
    interpolation
    smoothing
    synchronization

gauges/
    core
    analog
    digital
    tape
    progress
    graph
    map

render/
    gpu
    compositor
    preview
    export

project/
    model
    serialization
    autosave

platform/
    macos
    windows

utils/
```

Keep responsibilities clearly separated.

---

# 46. Rendering Model

Use a retained or scene-based rendering architecture.

Each gauge should produce a renderable scene/object.

Conceptually:

```text
Video Frame
     +
Gauge Scene
     +
Telemetry State
     ↓
GPU Renderer
     ↓
Preview Frame
```

This same scene representation should be convertible into the final render pipeline.

---

# 47. Performance Requirements

Performance is a primary product requirement.

The application must remain responsive while:

* Loading video
* Loading GPS files
* Scrubbing
* Playing video
* Rendering gauges
* Generating proxies
* Rendering final output

Never perform expensive processing on the UI thread.

Use Rust async/task/thread mechanisms appropriately.

---

# 48. Caching

Implement intelligent caching.

Cache:

* Video metadata
* GPS data
* Parsed telemetry
* Thumbnail frames
* Preview frames
* Proxy videos
* Rendered intermediate assets

Cache location should use the platform's application cache directory.

Allow the user to clear cache.

---

# 49. Background Processing

Use worker threads/tasks for:

* FIT parsing
* Video analysis
* Proxy generation
* Frame decoding
* Thumbnail generation
* GPS interpolation
* Final rendering

The UI should remain interactive.

---

# 50. Error Handling

Provide useful human-readable errors.

Examples:

### Invalid FIT

"Unable to read GPS file. The file may be corrupted or use an unsupported FIT profile."

### Missing telemetry

"This GPS recording does not contain heart-rate data."

### Video failure

"Unable to decode this video using the available codecs."

Do not expose Rust stack traces to normal users.

Log detailed technical information separately.

---

# 51. Logging

Implement structured logging using an appropriate Rust logging framework.

Example:

* tracing
* tracing-subscriber

Provide a developer log file.

Users should be able to access:

**Help → Open Logs**

---

# 52. FFmpeg Management

The application should detect whether FFmpeg is available.

Prefer bundling a known-compatible FFmpeg build with the application if licensing/distribution permits.

Otherwise provide configuration to locate FFmpeg.

At startup display:

* FFmpeg version
* Supported encoders
* Hardware acceleration availability

---

# 53. External Dependencies

Prefer mature, actively maintained Rust libraries.

Potential libraries to evaluate include:

* FIT parsing library
* GPX parser
* serde
* serde_json
* chrono
* geo
* image
* wgpu
* FFmpeg bindings or FFmpeg command integration
* tracing
* rayon
* tokio where appropriate

Do not add dependencies merely for convenience.

Evaluate maintenance status, licensing, performance and cross-platform compatibility before selecting each dependency.

---

# 54. macOS Application

The initial build should produce a normal macOS application.

Support:

* `.app`
* Apple Silicon
* Intel where practical

Provide:

* Application icon
* File associations
* Drag/drop
* Native dialogs
* Full-screen
* Retina scaling

---

# 55. Windows Application

The architecture must allow a Windows build without redesigning the application.

Support:

* Windows 10/11
* x64
* High DPI
* Native file dialogs
* Hardware encoding
* Drag/drop

Eventually provide:

`.exe`

and preferably an installer.

---

# 56. Keyboard Shortcuts

Implement professional editing shortcuts.

Examples:

Space:

Play/Pause

Left Arrow:

Previous frame

Right Arrow:

Next frame

Shift + Left:

Back several frames

Shift + Right:

Forward several frames

Home:

Beginning

End:

End

Delete:

Delete selected gauge

Ctrl/Cmd + Z:

Undo

Ctrl/Cmd + Shift + Z:

Redo

Ctrl/Cmd + S:

Save

Ctrl/Cmd + O:

Open

---

# 57. Undo / Redo

Implement application-wide undo/redo for important editing operations.

At minimum:

* Move gauge
* Resize gauge
* Delete gauge
* Add gauge
* Change gauge properties
* Change synchronization offset

---

# 58. Project State

Use a centralized application state model.

Avoid tightly coupling UI widgets to telemetry/video processing.

Changes should flow approximately:

```text
UI Command
    ↓
Application State
    ↓
Project Model
    ↓
Renderer
```

This makes future UI changes easier.

---

# 59. Preview Accuracy

The preview must accurately represent the final render.

Do not create a completely different gauge implementation for preview and final rendering.

The visual output should match as closely as possible.

Minor differences caused by video scaling or encoding are acceptable.

---

# 60. Initial MVP

The first working version should prioritize the core workflow.

MVP requirements:

1. Import MP4/MOV video
2. Import Garmin FIT file
3. Parse:

   * Speed
   * Altitude
   * Distance
   * Heart rate
   * Cadence
   * Power
   * Temperature
   * GPS
   * Date/time
4. Display video preview
5. Synchronize FIT with video
6. Display telemetry while scrubbing
7. Add gauges
8. At minimum implement:

   * Analog speedometer
   * Digital readout
   * Progress bar
   * Altitude graph
   * GPS route map
9. Drag/resize gauges
10. Preview gauges over video
11. Render final MP4 using FFmpeg
12. Preserve audio
13. Save/load project

Do not attempt to implement every advanced feature before the complete basic workflow works.

---

# 61. Development Priorities

Implement in this order:

### Phase 1

Application shell and native UI

### Phase 2

Video loading and playback

### Phase 3

FIT parsing and telemetry model

### Phase 4

Video/GPS synchronization

### Phase 5

GPU gauge renderer

### Phase 6

Gauge editor

### Phase 7

Timeline

### Phase 8

FFmpeg final rendering

### Phase 9

Proxy/caching optimization

### Phase 10

Advanced gauges and customization

### Phase 11

Windows support

---

# 62. Important Performance Principle

Do NOT make the mistake of rendering every video frame through a heavyweight UI framework.

The architecture should separate:

**Application UI**

from:

**Video/GPU rendering surface**

The video surface should be optimized for high-frequency frame updates.

Ideally:

```text
                 Rust Application
                       │
          ┌────────────┴────────────┐
          │                         │
      Native UI                 Video Engine
          │                         │
      Controls                    FFmpeg
      Panels                      Decoder
      Timeline                    Frame Cache
      Inspector                   GPU
          │                         │
          └────────────┬────────────┘
                       │
                  Render Surface
                       │
                 Video + Gauges
```

---

# 63. Design Philosophy

The application should NOT feel like a complicated professional NLE such as Premiere Pro.

It should feel like:

**"Drop your video + GPS file → arrange your gauges → render."**

The interface should be approachable for a normal cyclist, runner, windsurfer, hiker, motorsports enthusiast or action-camera user.

Prioritize:

* Simplicity
* Speed
* Visual feedback
* Large preview
* Minimal dialogs
* Drag-and-drop
* Fast scrubbing
* Excellent default templates

---

# 64. Visual Design

Use a modern dark professional interface.

Design inspiration:

* Final Cut Pro
* DaVinci Resolve
* Garmin Connect
* GoPro Quik
* modern automotive dashboards

However, do not copy their branding or exact UI.

Use:

* Dark neutral background
* Clear hierarchy
* Rounded controls where appropriate
* Subtle borders
* High-quality typography
* Large video canvas
* Compact inspector panels

Avoid excessive gradients, animations, shadows and unnecessary UI decoration.

---

# 65. Responsiveness

The UI should remain responsive during:

* Video playback
* Timeline scrubbing
* GPS processing
* Proxy generation
* Rendering

Show background progress indicators instead of freezing the application.

---

# 66. Extensibility

Design the gauge system as a plugin-like architecture.

Future gauges should be addable without modifying the core telemetry system.

For example:

```text
Telemetry
    ↓
Gauge Data Binding
    ↓
Gauge Renderer
```

A future gauge should be able to bind to:

* Speed
* Altitude
* Heart rate
* Cadence
* Power
* Temperature
* Distance
* Latitude
* Longitude
* Heading
* Time
* Custom telemetry

---

# 67. Future Features

Architect the application so the following can be added later:

* Weather overlays
* Wind speed/direction
* G-force
* Lap timing
* Segment timing
* Custom formulas
* Multiple GPS tracks
* Live camera metadata
* Garmin FIT developer fields
* ANT+ data
* GoPro metadata
* Insta360 metadata
* Telemetry from DJI
* Keyframe animation
* Gauge marketplace
* Custom gauge designer
* 3D route visualization
* 3D altitude profile
* Automatic gauge placement
* Social-media export presets

Do not implement these initially unless they are necessary for the architecture.

---

# 68. Code Quality

Write production-quality Rust.

Requirements:

* Strong typing
* Minimal unsafe Rust
* Clear module boundaries
* Error types using `thiserror` or equivalent
* Application errors using `anyhow` where appropriate
* Unit tests
* Integration tests
* Documentation for major modules
* No unnecessary unwraps in production paths
* No blocking operations on the UI thread

Use Rust formatting and linting:

* rustfmt
* clippy

---

# 69. Testing

Create tests for:

### FIT parsing

Verify telemetry extraction.

### Synchronization

Verify timestamp offsets.

### Interpolation

Verify values between GPS samples.

### Gauge rendering

Verify gauge properties.

### Project files

Verify save/load compatibility.

### Video

Verify FFmpeg command generation and rendering.

### Cross-platform

Test both macOS and Windows as development progresses.

---

# 70. Development Approach

Build the application incrementally.

After each major phase, produce a working application rather than creating thousands of lines of code before testing.

The application should compile and run after each major milestone.

Do not create placeholder architecture that cannot actually be executed.

When implementing a subsystem, build a minimal working version first and then optimize it.

---

# 71. First Development Milestone

The first milestone should produce a functional desktop application that can:

1. Launch on macOS
2. Display the native application UI
3. Import a video
4. Display video metadata
5. Play the video
6. Scrub the timeline
7. Import a FIT file
8. Parse GPS data
9. Display telemetry values
10. Synchronize GPS and video
11. Display a simple speed gauge over the video
12. Move and resize the gauge
13. Export the result through FFmpeg

Only after this complete end-to-end workflow works should advanced gauge styles be implemented.

---

# 72. Final Product Goal

The finished application should be a fast, polished native desktop application that allows an action-sports or GPS-video user to quickly transform ordinary video into professional telemetry footage.

The ideal workflow is:

```text
DROP VIDEO
     ↓
DROP GPS/FIT FILE
     ↓
AUTOMATIC TIME SYNC
     ↓
CHOOSE GAUGE TEMPLATE
     ↓
DRAG GAUGES ONTO VIDEO
     ↓
SCRUB / PREVIEW
     ↓
ADJUST
     ↓
RENDER
     ↓
PROFESSIONAL GPS TELEMETRY VIDEO
```

The most important engineering priorities are:

1. **Video playback performance**
2. **Smooth timeline scrubbing**
3. **Fast GPS synchronization**
4. **GPU-accelerated gauge rendering**
5. **Accurate telemetry interpolation**
6. **Excellent FFmpeg integration**
7. **Responsive native UI**
8. **Cross-platform macOS/Windows architecture**
9. **Simple user experience**
10. **High-quality final video output**

Build the application as a serious production-quality desktop application rather than a prototype or web wrapper.

When there is a tradeoff between visual complexity and performance, choose **performance**.

When there is a tradeoff between feature count and a polished core workflow, choose the **polished core workflow**.
