mod audio;
mod audio_frame_buffer;
mod color;
mod infrared;
mod rtsp_publisher;

use anyhow::Context;
use clap::Parser;
use kinect_v2::Kinect;

use crate::audio::spawn_audio_pipeline;
use crate::color::spawn_color_pipeline;
use crate::infrared::spawn_infra_pipeline;
use crate::rtsp_publisher::RtspPublisher;
use serde::Serialize;

#[derive(Serialize)]
struct RtspAuthHandoff<'a> {
    username: &'a str,
    password: &'a str,
}

fn write_rtsp_auth_handoff(username: Option<&str>, password: Option<&str>) {
    let path = std::env::temp_dir().join("ai-baby-monitor-rtsp-auth.json");
    match (username, password) {
        (Some(u), Some(p)) => {
            let payload = RtspAuthHandoff {
                username: u,
                password: p,
            };
            match serde_json::to_vec_pretty(&payload) {
                Ok(bytes) => {
                    if let Err(e) = std::fs::write(&path, bytes) {
                        log::warn!("Failed to write RTSP auth handoff file at {path:?}: {e}");
                    } else {
                        log::info!("Wrote RTSP auth handoff to {path:?}");
                    }
                }
                Err(e) => log::warn!("Failed to serialize RTSP auth handoff JSON: {e}"),
            }
        }
        _ => {
            // Remove stale handoff file if present
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::debug!("Could not remove RTSP auth handoff file {path:?}: {e}");
                }
            } else {
                log::info!("Removed existing RTSP auth handoff file {path:?}");
            }
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "kinect-rtsp",
    about = "Kinect RTSP server with optional Basic Auth"
)]
struct Cli {
    /// Username for RTSP Basic Auth
    #[arg(long)]
    rtsp_username: Option<String>,

    /// Password for RTSP Basic Auth
    #[arg(long)]
    rtsp_password: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::init();

    // Parse CLI
    let args = Cli::parse();

    // Persist credentials for the WebRTC bridge (optional)
    write_rtsp_auth_handoff(args.rtsp_username.as_deref(), args.rtsp_password.as_deref());

    start_kinect_capture(args.rtsp_username, args.rtsp_password)?;

    // Wait for Ctrl-C; when received, abort the server task and await it.
    log::info!("Press Ctrl-C to exit...");
    tokio::signal::ctrl_c().await?;
    log::info!("Ctrl-C received — shutting down services...");

    Ok(())
}

pub fn start_kinect_capture(
    rtsp_username: Option<String>,
    rtsp_password: Option<String>,
) -> anyhow::Result<()> {
    {
        let kinect = Kinect::new().context("Failed to create Kinect instance")?;
        // Small wait loop to allow the device to become available
        for _ in 0..10 {
            if kinect.is_available()? {
                break;
            }
            log::info!("Waiting for Kinect device to become available...");
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        if !kinect.is_available()? {
            return Err(anyhow::anyhow!("Kinect device is not available"));
        }
    }

    log::info!("Starting RTSP server...");
    // Start RTSP server (GStreamer) and publish Kinect streams
    let rtsp = RtspPublisher::start(rtsp_username.as_deref(), rtsp_password.as_deref())?;

    log::info!("RTSP server started successfully on port 8554");

    // Start Kinect capture and push raw frames to RTSP appsrcs
    spawn_color_pipeline(rtsp.clone());
    spawn_infra_pipeline(rtsp.clone());
    spawn_audio_pipeline(rtsp.clone());

    log::info!("All pipelines started, waiting for streams to initialize...");

    // Log RTSP URLs for easy access
    log::info!("RTSP streams available:");
    if let (Some(u), Some(_)) = (rtsp_username.as_deref(), rtsp_password.as_deref()) {
        log::info!("  Color:    rtsp://{u}:***@localhost:8554/color");
        log::info!("  Infrared: rtsp://{u}:***@localhost:8554/infrared");
    } else {
        log::info!("  Color:    rtsp://localhost:8554/color");
        log::info!("  Infrared: rtsp://localhost:8554/infrared");
    }
    log::info!("");
    log::info!("To view streams in VLC:");
    log::info!("  1. Open VLC Media Player");
    log::info!("  2. Go to Media > Open Network Stream");
    log::info!("  3. Enter one of the URLs above");
    log::info!("  4. Click Play");
    log::info!("");

    Ok(())
}
