# Kinect RTSP (Kinect V2 → RTSP)

Kinect RTSP streams color, infrared and audio from a Kinect V2 sensor to a local RTSP server using GStreamer.

This project requires the official Microsoft Kinect for Windows SDK 2.0 and runs on Windows only. It uses the `kinect-v2` Rust bindings for device access and GStreamer to publish appsrcs as RTSP streams.

## Quick summary
- Platform: Windows (Kinect V2 SDK is Windows-only)
- Inputs: Kinect V2 sensor
- Outputs: RTSP streams (color, infrared, audio)
- Build system: Rust/Cargo

## Requirements
- Windows (tested on 64-bit)
- Kinect V2 sensor + Kinect Adapter
- Kinect for Windows SDK 2.0 installed (always required)
- GStreamer "Development" runtime for MSVC x86_64 installed when building from source:
  - Install the `gstreamer-1.0-devel-msvc-x86_64-<version>.msi` (and matching runtime) for your GStreamer version.
  - When running from a release ZIP, the repository owner will provide a ZIP containing the necessary GStreamer runtime files; Kinect SDK still must be installed separately.
- Rust toolchain (recommended stable recent release)

## Build
Open a PowerShell prompt (pwsh) and run:

```powershell
# optional: set verbose logging
$env:RUST_LOG = "info"
# Build release binary
cargo build --release
```

Notes:
- Ensure the GStreamer MSVC runtime/bin is on your `PATH` or installed system-wide before running the built binary.
- If you get GStreamer-related errors at runtime, verify the installed GStreamer package matches MSVC x86_64 and that its `bin` directory is in `PATH`.

## Run (development)
Run the server from source with optional Basic Auth and port flags:

```powershell
# Run without auth, default port 8554
cargo run --release

# Run with Basic Auth
cargo run --release -- --username myuser --password mypass --port 8554
```

If you prefer to run the prebuilt release ZIP provided by the maintainer, extract the ZIP and run the `kinect-rtsp.exe` included in the archive. The release ZIP will contain GStreamer dependencies so the end user doesn't need to install GStreamer separately — but the Kinect V2 SDK must still be installed.

## CLI Options
The binary accepts these flags:

- `--username <username>`  Optional RTSP Basic Auth username
- `--password <password>`  Optional RTSP Basic Auth password
- `--port <port>`          RTSP server port, defaults to `8554`

Example:

```powershell
.\	arget\release\kinect-rtsp.exe --username alice --password s3cret --port 8554
```

## RTSP URLs
When the server starts it will log RTSP URLs. Example (no auth):

- rtsp://localhost:8554/color
- rtsp://localhost:8554/infrared

When Basic Auth is enabled, use an authenticated URL (VLC or other clients will prompt for credentials), for example:

- rtsp://alice:***@localhost:8554/color

## Viewing streams
Open VLC Media Player > Media > Open Network Stream, then paste one of the RTSP URLs above and Play.

## Logging
This project uses `env_logger`. To see informational logs at runtime, set `RUST_LOG` before running:

```powershell
$env:RUST_LOG = "info"
cargo run --release -- --port 8554
```

## Troubleshooting
- "Kinect device is not available": ensure the Kinect sensor is connected and the Kinect SDK 2.0 is installed.
- GStreamer errors: make sure you installed the MSVC x86_64 GStreamer package and that its `bin` directory is available on `PATH`.
- Permission/driver issues: run PowerShell elevated if Windows blocks device access.

## Development notes
- The project spawns three pipelines: color, infrared and audio. The RTSP server is implemented with GStreamer appsrcs.
- See `src/main.rs` for CLI flags and startup flow.

## Packaging / Release
When publishing a release for end users, include a ZIP containing:
- The compiled `kinect-rtsp.exe` (release build)
- The GStreamer runtime DLLs and binaries required for the executable (so end users don't have to install GStreamer)

Do NOT include the Kinect SDK in the ZIP — the Kinect for Windows SDK 2.0 must be installed by the user because of licensing and installer requirements.

Add a clear note in the release and project description that the Kinect SDK is required and is not bundled.

## License
This project follows the same license as its dependencies; verify `Cargo.toml` / `LICENSE` in the root for exact terms.

## Acknowledgements
- Microsoft Kinect for Windows SDK 2.0
- The `kinect-v2` Rust bindings project
- GStreamer project for RTSP and media handling

