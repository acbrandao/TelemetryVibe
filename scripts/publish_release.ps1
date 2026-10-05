# Publishes the Windows binaries in dist\TelemetryVibe as a GitHub Release, tagged v<version> from Cargo.toml.
# Requires the GitHub CLI (winget install GitHub.cli) logged in with `gh auth login`.
# If the release already exists, its binaries are replaced with the current ones.
#
#   powershell -ExecutionPolicy Bypass -File scripts\publish_release.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\publish_release.ps1 -Build            # rebuild x64 + ARM64 first
#   powershell -ExecutionPolicy Bypass -File scripts\publish_release.ps1 -Version 0.2.0 -Draft
param(
    [string]$Version,          # defaults to the version in Cargo.toml
    [switch]$Build,            # rebuild x64 and ARM64 before publishing
    [switch]$Draft,
    [switch]$Prerelease,
    [string]$Notes             # release notes; defaults to GitHub's generated notes
)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw "GitHub CLI not found. Install it with 'winget install GitHub.cli', then run 'gh auth login'."
}
gh auth status | Out-Null
if ($LASTEXITCODE -ne 0) { throw "GitHub CLI is not logged in. Run 'gh auth login' and try again." }

if (-not $Version) {
    $match = Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
    if (-not $match) { throw "Couldn't find a version in Cargo.toml. Pass -Version." }
    $Version = $match.Matches[0].Groups[1].Value
}
$tag = "v$($Version.TrimStart('v'))"

# The release tag points at HEAD, so HEAD must be committed and pushed.
if (git status --porcelain --untracked-files=no) {
    throw "You have uncommitted changes. Commit or stash them before publishing."
}
$commit = git rev-parse HEAD
git fetch --quiet origin
if (-not (git branch -r --contains $commit)) {
    throw "Commit $($commit.Substring(0, 7)) isn't on GitHub yet. Push it before publishing."
}

if ($Build) {
    foreach ($target in "x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc") {
        & (Join-Path $PSScriptRoot "build_windows.ps1") -Target $target
    }
}

$assets = @(Get-ChildItem "dist\TelemetryVibe\TelemetryVibe-*.exe" -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -match '^TelemetryVibe-(x64|arm64)\.exe$' })
if ($assets.Count -eq 0) {
    throw "No binaries found in dist\TelemetryVibe. Run scripts\build_windows.ps1 first, or pass -Build."
}
foreach ($asset in $assets) {
    Write-Host "==> $($asset.Name)  ($([math]::Round($asset.Length / 1MB, 1)) MB, built $($asset.LastWriteTime))"
}

$ErrorActionPreference = "Continue"   # gh writes "release not found" to stderr; don't treat that as fatal
gh release view $tag 2>$null | Out-Null
$exists = $LASTEXITCODE -eq 0
$ErrorActionPreference = "Stop"
if ($exists) {
    Write-Host "==> Release $tag already exists; replacing its binaries..."
    gh release upload $tag $assets.FullName --clobber
} else {
    Write-Host "==> Creating release $tag at $($commit.Substring(0, 7))..."
    $ghArgs = @($tag) + $assets.FullName + @("--target", $commit, "--title", "TelemetryVibe $tag")
    if ($Notes) { $ghArgs += @("--notes", $Notes) } else { $ghArgs += "--generate-notes" }
    if ($Draft) { $ghArgs += "--draft" }
    if ($Prerelease) { $ghArgs += "--prerelease" }
    gh release create @ghArgs
}
if ($LASTEXITCODE -ne 0) { throw "Publishing release $tag failed." }

gh release view $tag --json url --jq .url
