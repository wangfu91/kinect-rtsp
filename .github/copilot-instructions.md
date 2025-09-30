## Purpose

This file gives concise, actionable guidance for AI coding agents working on the `kinect-rtsp` repo so they can be productive immediately.

## Big picture (what this repo does)

- Windows-only Rust app that reads Kinect V2 (color, infrared, audio) and publishes them as RTSP streams using GStreamer.
- Main components:
  - `src/main.rs` — CLI, startup and orchestration (Tokio runtime).
  - `src/rtsp_publisher.rs` — GStreamer RTSP server wiring, `RtspPublisher` API and auth.
  - `src/color.rs` / `src/infrared.rs` — spawn pipelines that capture frames from Kinect and call `RtspPublisher::send_*`.
  - `src/audio.rs` and `src/audio_frame_buffer.rs` — audio capture, buffering, and conversion to f32 samples.

Design notes the agent should respect:

- GStreamer is used via `appsrc` elements; frames/audio are pushed into appsrcs from Rust code. Pixel/audio formats are important (see `send_color_yuy2`, `send_infra_bgra`, `send_audio_f32`).
- RtspPublisher runs a GLib main loop in a separate thread while the rest of the app uses Tokio — don't change threading model without understanding this interaction.
- The repo patches the `kinect-v2` crate to a git URL in `Cargo.toml` — changes to Kinect bindings may require adjusting that repo.

## Key files to open first

- `src/main.rs` — CLI flags, startup order, how pipelines are spawned.
- `src/rtsp_publisher.rs` — where RTSP mounts are created, how appsrcs are configured, and how auth is implemented.
- `src/color.rs`, `src/infrared.rs`, `src/audio.rs`, `src/audio_frame_buffer.rs` — actual capture and format expectations.
- `run.ps1` and `build.rs` — run script behavior and why `run.ps1` is copied next to the exe.
- `Cargo.toml` — dependencies and a `patch.crates-io` entry pointing `kinect-v2` to a git repo.

## Build / run / debug workflows (concrete)

- Prerequisites: Kinect for Windows SDK 2.0 installed and the MSVC x86_64 GStreamer runtime. The GStreamer `bin` must be on `PATH` and plugins available via `GST_PLUGIN_PATH`.
- Quick build & run (release):

  cargo build --release

  # run the produced binary (or use the run.ps1 wrapper next to the exe)

  .\target\release\kinect-rtsp.exe --port 8554

- Development run (with GStreamer debug and Rust logs):
  $env:GST_DEBUG = "\*:3"
  $env:RUST_LOG = "debug"
  cargo run

- The repository provides `run.ps1`. `build.rs` copies `run.ps1` next to the built exe so users can run the wrapper which sets GStreamer env vars when `GSTREAMER_1_0_ROOT_MSVC_X86_64` is present.

## Environment variables and runtime expectations

- GSTREAMER_1_0_ROOT_MSVC_X86_64: if set to a GStreamer MSVC runtime, `run.ps1` will add its `bin` to `PATH` and set `GST_PLUGIN_PATH`.
- GST_DEBUG: controls GStreamer debug level.
- RUST_LOG: controls Rust logging (env_logger is used; default level is `info` when not set).

## Project-specific conventions and patterns

- Pixel formats and sizes are strict: color uses YUY2 (1920x1080@30) and infrared uses BGRA (512x424@30). When changing capture code, keep these formats or update `rtsp_publisher::create_factory` caps accordingly.
- `RtspPublisher` exposes small, focused methods to push data: `send_color_yuy2`, `send_infra_bgra`, `send_audio_f32`. Use those rather than directly touching GStreamer internals.
- Appsrc configuration choices matter: `is-live=true`, `format=time`, `do-timestamp=true`, `set_block(true)` and `set_max_bytes(...)` are used to control backpressure. Preserve these semantics when editing streaming code.
- When adding or requiring a new GStreamer element, update the explicit checks in `rtsp_publisher::start` (`check_gst_element(...)`) so runtime errors are clear.
- Auth is stored in a `OnceCell` and validated inside a small `auth` module — auth is enabled only when both `--username` and `--password` are provided at start.

## Integration points & external deps

- Kinect SDK 2.0 (native dependency) — device availability is checked at startup in `main.rs`.
- GStreamer MSVC runtime — required plugins: `appsrc`, `videoconvert`, `openh264enc`, `h264parse`, `rtph264pay`, `queue`, `audioresample`, `audioconvert`, `opusenc`, `rtpopuspay`. Missing elements fail early via `check_gst_element`.
- The `kinect-v2` Rust crate is patched to a git repo in `Cargo.toml` — changes in Kinect behavior often require changes in that dependency.

## Safe change guidelines (what to check after edits)

- If you change a pipeline string in `create_factory`, test end-to-end with a local GStreamer runtime and a client (ffplay or VLC).
- Keep `gst::init()` called before any GStreamer usage (currently in `RtspPublisher::start`).
- Modifying audio sample format: `send_audio_f32` converts f32->[i16] and pushes same bytes to both /color and /infrared audio appsrcs. If you change sample rates/formats update caps in `create_factory`.
- If you change threading (e.g., run GLib main loop in-process), make sure it doesn't block Tokio or cause deadlocks.

## Troubleshooting quick hits (concrete)

- If streams don't start: verify Kinect device is available (Windows Device Manager) and Kinect SDK installed.
- If GStreamer plugins missing: install the MSVC x86_64 runtime and set `GSTREAMER_1_0_ROOT_MSVC_X86_64` or add `bin` to PATH.
- For verbose GStreamer logs: set `$env:GST_DEBUG = "*:4"` (or higher) before running.

## Small PR checklist for maintainers

- Prefer small, Windows-friendly changes and document any added native requirements in `README.md`.
- When adding a new GStreamer element or pipeline variant, update `rtsp_publisher::start`'s element checks and add a short note in `README.md`.

## Where to look next (helpful entry points for tasks)

- Add/adjust pipeline caps and encoder params: `src/rtsp_publisher.rs::create_factory`.
- Modify capture frame handling: `src/color.rs` / `src/infrared.rs`.
- Change audio buffering / encoding: `src/audio.rs` and `src/audio_frame_buffer.rs`.

---

If anything above is unclear or you want examples expanded (for example: how to add a new RTSP mount, or how to add unit tests), tell me which section to expand and I will iterate.
