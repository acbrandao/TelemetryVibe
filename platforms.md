# Building TelemetryVibe on Windows, macOS and Linux

Each platform has one setup script. It installs everything TelemetryVibe needs and then builds the app.
You can run it again at any time: tools that are already installed are skipped.

| Platform | One command | Result |
|---|---|---|
| macOS | `scripts/setup_macos.sh` | `dist/TelemetryVibe.app` |
| Windows | `powershell -ExecutionPolicy Bypass -File scripts\setup_windows.ps1` | `dist\TelemetryVibe\telemetryvibe.exe` |
| Linux | `scripts/setup_linux.sh` | `dist/TelemetryVibe/telemetryvibe` |

Every platform needs the same three things:

1. **A C/C++ toolchain** (Xcode Command Line Tools, Visual Studio Build Tools, or gcc).
2. **Rust**, stable 1.88 or newer, installed with [rustup](https://rustup.rs).
3. **FFmpeg** (`ffmpeg` and `ffprobe`). The app runs them to read and write video. It finds them on
   your `PATH`, next to the app, or wherever you point **Settings → FFmpeg**.

The first build takes a few minutes. Later builds are much faster.

---

## macOS

Works on macOS 11 or newer, on Apple Silicon and Intel.

### Quick way

Open **Terminal**, go to the project folder, and run:

```bash
scripts/setup_macos.sh
```

The first time, a dialog asks you to install the Xcode Command Line Tools. Click **Install**, wait
for it to finish, then run the script again.

When it's done:

```bash
open dist/TelemetryVibe.app
```

To install it, drag `dist/TelemetryVibe.app` into `/Applications`.

### Manual way

```bash
xcode-select --install                                           # compiler and linker
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust; then open a new terminal
brew install ffmpeg                                              # needs Homebrew: https://brew.sh
scripts/bundle_macos.sh                                          # builds dist/TelemetryVibe.app
```

### Options

```bash
cargo run --release                                   # run straight from source, without building a bundle
UNIVERSAL=1 scripts/bundle_macos.sh                   # one app for both Apple Silicon and Intel
FFMPEG_DIR=/path/to/static scripts/bundle_macos.sh    # put ffmpeg/ffprobe inside the app
```

`FFMPEG_DIR` needs a **static** FFmpeg build. Homebrew's FFmpeg won't work when copied to another Mac.

---

## Windows

Works on Windows 10 and 11, on x64 and ARM64.

### Quick way

Open **PowerShell**, go to the project folder, and run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup_windows.ps1
```

The script uses `winget` to install:

- Visual Studio 2022 Build Tools with the C++ workload (about 3 GB; this step takes the longest)
- Rust (rustup)
- FFmpeg

Then it builds the app. Approve any Windows permission (UAC) prompts that appear.

When it's done:

```powershell
dist\TelemetryVibe\telemetryvibe.exe
```

To build for ARM64 (for example, Surface or Snapdragon laptops):

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup_windows.ps1 -Target aarch64-pc-windows-msvc
```

If `winget` isn't found, install **App Installer** from the Microsoft Store and try again.

### Manual way

1. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   and tick **Desktop development with C++**.
2. Install Rust from [rustup.rs](https://rustup.rs).
3. Install FFmpeg (`winget install Gyan.FFmpeg`), or unzip a build into `C:\ffmpeg\bin`.
4. Open a **new** PowerShell window and run:

   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts\build_windows.ps1
   ```

### Shipping FFmpeg with the app

Copy `ffmpeg.exe` and `ffprobe.exe` into `dist\TelemetryVibe\`, next to `telemetryvibe.exe`. The app uses
them automatically, so the whole folder can be zipped and run on another PC.

---

## Linux

Works with Ubuntu/Debian (`apt`), Fedora (`dnf`) and Arch (`pacman`), on X11 or Wayland.

### Quick way

Open a terminal, go to the project folder, and run:

```bash
scripts/setup_linux.sh
```

It asks for your password (for `sudo`) to install system packages, then installs Rust and builds the app.

When it's done:

```bash
dist/TelemetryVibe/telemetryvibe
```

To also add TelemetryVibe to your app menu and open `.gpsvideo` files with it:

```bash
INSTALL=1 scripts/setup_linux.sh
```

This installs into `~/.local/bin` and `~/.local/share`, so no root access is needed for this step.
Make sure `~/.local/bin` is on your `PATH`.

### Manual way

Install the system packages for your distribution:

```bash
# Ubuntu / Debian
sudo apt install build-essential pkg-config curl libxkbcommon-dev libwayland-dev libx11-dev \
  libxcursor-dev libxrandr-dev libxi-dev libvulkan1 mesa-vulkan-drivers \
  xdg-desktop-portal xdg-desktop-portal-gtk ffmpeg

# Fedora
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config curl libxkbcommon-devel wayland-devel \
  libX11-devel libXcursor-devel libXrandr-devel libXi-devel vulkan-loader mesa-vulkan-drivers \
  xdg-desktop-portal xdg-desktop-portal-gtk ffmpeg

# Arch
sudo pacman -S --needed base-devel curl libxkbcommon wayland libx11 libxcursor libxrandr libxi \
  vulkan-icd-loader xdg-desktop-portal xdg-desktop-portal-gtk ffmpeg
```

Then install Rust and build:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # then open a new terminal
scripts/build_linux.sh                                           # INSTALL=1 to add to the app menu
```

### Linux notes

- **Fedora FFmpeg:** Fedora's own `ffmpeg-free` package has no H.264/H.265 encoder. For full FFmpeg,
  enable [RPM Fusion](https://rpmfusion.org/Configuration) and run `sudo dnf install ffmpeg --allowerasing`.
- **Open/Save dialogs** use the desktop portal (`xdg-desktop-portal`). If they don't appear, make sure
  a portal backend for your desktop is installed (`-gtk`, `-gnome` or `-kde`), then log out and back in.
- **Graphics:** the app draws with Vulkan, or OpenGL if Vulkan isn't available. On a blank window or a
  graphics error, install your GPU's Vulkan driver (e.g. `vulkan-radeon` or `vulkan-intel` on Arch).

---

## Check that it worked

```bash
cargo run --release --example make_sample   # creates sample/sample_ride.mp4 and sample/sample_ride.fit
```

Start the app, drag both sample files onto the window, choose **Cycling** in the template picker, and
press **Render Video**.

To run the tests (the end-to-end tests are skipped if FFmpeg is missing):

```bash
cargo test --release
```

## Troubleshooting

| Problem | Fix |
|---|---|
| `cargo: command not found` | Open a new terminal after installing Rust, or run `source ~/.cargo/env` (macOS/Linux). |
| Rust errors about `edition2024` or `let` chains | Your Rust is too old. Run `rustup update stable`. |
| `linker 'link.exe' not found` (Windows) | The C++ build tools are missing. Re-run the setup script or install **Desktop development with C++**. |
| `linker 'cc' not found` (Linux) | Install `build-essential` (apt), `gcc` (dnf) or `base-devel` (pacman). |
| App says FFmpeg wasn't found | Install FFmpeg, or set its folder in **Settings → FFmpeg**. Run `ffmpeg -version` to check that it's on your `PATH`. |
| macOS says the app "can't be opened" | Right-click the app, choose **Open**, then confirm. Local builds are signed ad hoc, not notarized. |
| Windows SmartScreen warning | Click **More info → Run anyway**. Local builds aren't code-signed. |

Logs are in the app's data folder. Open it with **Help → Open Logs**.
