## Kinect RTSP (Kinect V2 → RTSP) 🎥📡

Turn a Kinect V2 sensor into a small RTSP camera. This tool reads color, infrared and audio from a Kinect V2 and publishes them as RTSP streams using GStreamer.

This project is part of a larger effort to build a baby-monitoring system using the Kinect V2 sensor.

Platform: **Windows x64 only**.

## Table of contents
- [Kinect RTSP (Kinect V2 → RTSP) 🎥📡](#kinect-rtsp-kinect-v2--rtsp-)
- [Table of contents](#table-of-contents)
- [Prerequisites](#prerequisites)
- [CLI options](#cli-options)
- [Quick start](#quick-start)
- [RTSP URLs 📡](#rtsp-urls-)
- [Troubleshooting 🧰](#troubleshooting-)
- [Development notes 🛠️](#development-notes-️)
- [Contributing](#contributing)
- [License](#license)
- [Acknowledgements](#acknowledgements)

---

## Prerequisites

- [Kinect for Windows SDK 2.0](https://www.microsoft.com/en-us/download/details.aspx?id=44561).
- [GStreamer MSVC x86_64 runtime](https://gstreamer.freedesktop.org/download/#windows) — ensure the `bin` directory of the runtime is on `PATH`.

## CLI options
- `--username <username>`  Optional RTSP Basic Auth username.
- `--password <password>`  Optional RTSP Basic Auth password.
- `--port <port>`          RTSP server port (default: `8554`).

## Quick start

1. Install prerequisites.
2. Build the release binary:

```powershell
cargo build --release
```

3. Run the server:

```powershell
# start without auth on default port 8554
.\target\release\kinect-rtsp.exe

# start with Basic Auth and custom port
.\target\release\kinect-rtsp.exe --username alice --password s3cret --port 8554
```

4. Open a client (VLC, ffplay, etc.) and open one of the RTSP URLs listed below.


## RTSP URLs 📡
When the server starts it will log RTSP URLs. Typical examples:

- rtsp://localhost:8554/color
- rtsp://localhost:8554/infrared

If Basic Auth is enabled the client will be prompted for credentials (or you can use an authenticated URL):

- rtsp://alice:***@localhost:8554/color

## Troubleshooting 🧰

- Kinect device is not available:
	- Ensure the Kinect sensor is connected and powered.
	- Verify Kinect SDK 2.0 is installed and device appears in Windows Device Manager.
	- Try rebooting after SDK installation.

- GStreamer errors or missing plugins:
	- Confirm you installed the MSVC x86_64 GStreamer runtime, not the MinGW variant.
	- Make sure the runtime `bin` folder is on `PATH` (see installation tip above).
	- Run the binary from an elevated PowerShell if you face permission issues.

- Client can't open the stream:
	- Try `ffplay` to rule out client issues: `ffplay rtsp://localhost:8554/color`
	- Check application logs — the program prints pipeline and RTSP server status on startup.

## Development notes 🛠️

- The program spawns three GStreamer pipelines (color, infrared and audio) and publishes them with `appsrc` to a local RTSP server.
- See `src/main.rs` for startup flow and CLI flags. Other key files:
	- `src/color.rs` — color pipeline handling
	- `src/infrared.rs` — infrared pipeline handling
	- `src/audio.rs` / `src/audio_frame_buffer.rs` — audio capture and buffering
	- `src/rtsp_publisher.rs` — GStreamer RTSP server wiring

- To increase GStreamer logging during development:

```powershell
# set verbose GStreamer debug output for current session
$env:GST_DEBUG = "*:3"
cargo run
```

## Contributing

Contributions, bug reports and PRs are welcome. Please:

1. Open an issue describing the problem or feature.
2. Create a small, focused PR with tests or reproduction steps when possible.
3. Keep changes Windows-friendly and document any new external requirements.

## License
MIT — see [LICENSE](./LICENSE).

## Acknowledgements

- Microsoft Kinect for Windows SDK 2.0
- GStreamer project

