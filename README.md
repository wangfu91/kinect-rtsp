# Kinect RTSP (Kinect V2 → RTSP) 🎥📡

Kinect RTSP streams color, infrared and audio from a Kinect V2 sensor to a local RTSP server using GStreamer. Think of it as turning your Kinect into a tiny network camera!

This project requires the official Microsoft Kinect for Windows SDK 2.0 and runs on Windows only. It uses the [kinect-v2-rs](https://github.com/wangfu91/kinect-v2-rs) Rust bindings for device access and GStreamer to publish appsrcs as RTSP streams.

## Quick summary 🚀
- Platform: Windows (Kinect V2 SDK is Windows-only) 
- Inputs: Kinect V2 sensor 
- Outputs: RTSP streams (color, infrared, audio)
- Build system: Rust/Cargo

## Requirements ⚠️
- Windows x64
- Kinect V2 sensor + Kinect Adapter
- [Kinect for Windows SDK 2.0](https://www.microsoft.com/en-us/download/details.aspx?id=44561) installed

## Build 🛠️
Open a PowerShell prompt (pwsh) and run:

```powershell
# Build release binary
cargo build --release
```

Notes:
- Ensure the GStreamer MSVC runtime/bin is on your `PATH` or installed system-wide before running the built binary. 
- If you get GStreamer-related errors at runtime, verify the installed GStreamer package matches MSVC x86_64 and that its `bin` directory is in `PATH`. 

## Run (development) ▶️
Run the server from source with optional Basic Auth and port flags:

```powershell
# Run without auth, default port is 8554
cargo run --release

# Run with Basic RTSP Auth
cargo run --release -- --username myuser --password mypass --port 8554
```

If you prefer to run the prebuilt release ZIP provided by the maintainer, extract the ZIP and run the `kinect-rtsp.exe` included in the archive. The release ZIP will contain GStreamer dependencies so the end user doesn't need to install GStreamer separately — but the Kinect V2 SDK must still be installed. 

## CLI Options 🔧
The binary accepts these flags:

- `--username <username>`  Optional RTSP Basic Auth username 
- `--password <password>`  Optional RTSP Basic Auth password 
- `--port <port>`          RTSP server port, defaults to `8554` 

Example:

```powershell
.\kinect-rtsp.exe --username alice --password s3cret --port 8554
```

## RTSP URLs 📡
When the server starts it will log RTSP URLs. Example (no auth):

- rtsp://localhost:8554/color
- rtsp://localhost:8554/infrared

When Basic Auth is enabled, use an authenticated URL (VLC or other clients will prompt for credentials), for example:

- rtsp://alice:***@localhost:8554/color 

## Viewing streams ▶️
Open VLC Media Player > Media > Open Network Stream, then paste one of the RTSP URLs above and Play. 

## Troubleshooting 🧰
- "Kinect device is not available": ensure the Kinect sensor is connected and the Kinect SDK 2.0 is installed. 
- GStreamer errors: make sure you installed the MSVC x86_64 GStreamer package and that its `bin` directory is available on `PATH`. 

## Development notes 🛠️
- The project spawns three pipelines: color, infrared and audio. The RTSP server is implemented with GStreamer appsrcs. 
- See `src/main.rs` for CLI flags and startup flow. 

## License 🧾
MIT, see [LICENSE](./LICENSE) file for details.

## Acknowledgements 🙏
- [Microsoft Kinect for Windows SDK 2.0](https://www.microsoft.com/en-us/download/details.aspx?id=44561)
- The [kinect-v2-rs](https://github.com/wangfu91/kinect-v2-rs) Rust bindings project 
- [GStreamer](https://gstreamer.freedesktop.org/) project for RTSP and media handling

