mod audio;
mod audio_frame_buffer;
mod color;
mod infrared;
mod rtsp_publisher;

use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use kinect_v2::Kinect;
use tokio::time::sleep;

use crate::audio::spawn_audio_pipeline;
use crate::color::spawn_color_pipeline;
use crate::infrared::spawn_infra_pipeline;
use crate::rtsp_publisher::RtspPublisher;

#[derive(Debug, Parser)]
#[command(
    name = "kinect-rtsp",
    about = "Kinect RTSP server with optional Basic Auth"
)]
struct Cli {
    /// Optional, username for RTSP Basic Auth
    #[arg(long)]
    username: Option<String>,

    /// Optional, password for RTSP Basic Auth
    #[arg(long)]
    password: Option<String>,

    /// Optional, port for RTSP server,
    /// Default to 8554 if not specified
    #[arg(long, default_value_t = 8554)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::init();

    // Parse CLI
    let args = Cli::parse();

    start_kinect_capture(args.username, args.password, args.port).await?;

    // Wait for Ctrl-C; when received, abort the server task and await it.
    log::info!("Press Ctrl-C to exit...");
    tokio::signal::ctrl_c().await?;
    log::info!("Ctrl-C received — shutting down services...");

    Ok(())
}

pub async fn start_kinect_capture(
    rtsp_username: Option<String>,
    rtsp_password: Option<String>,
    rtsp_port: u16,
) -> anyhow::Result<()> {
    {
        let kinect = Kinect::new().context("Failed to create Kinect instance")?;
        // Small wait loop to allow the device to become available
        for _ in 0..10 {
            if kinect.is_available()? {
                break;
            }
            log::debug!("Waiting for Kinect device to become available...");
            sleep(Duration::from_millis(200)).await;
        }

        if !kinect.is_available()? {
            return Err(anyhow::anyhow!("Kinect device is not available"));
        }
    }

    log::info!("Starting RTSP server...");
    // Start RTSP server (GStreamer) and publish Kinect streams
    let rtsp = RtspPublisher::start(
        rtsp_username.as_deref(),
        rtsp_password.as_deref(),
        rtsp_port,
    )?;

    log::info!("RTSP server started successfully on port {rtsp_port}");

    // Start Kinect capture and push raw frames to RTSP appsrcs
    spawn_color_pipeline(rtsp.clone());
    spawn_infra_pipeline(rtsp.clone());
    spawn_audio_pipeline(rtsp.clone());

    log::info!("All pipelines started, waiting for streams to initialize...");

    // Log RTSP URLs for easy access
    log::info!("RTSP streams available:");
    if let (Some(u), Some(_)) = (rtsp_username.as_deref(), rtsp_password.as_deref()) {
        log::info!("  Color:    rtsp://{u}:***@localhost:{rtsp_port}/color");
        log::info!("  Infrared: rtsp://{u}:***@localhost:{rtsp_port}/infrared");
    } else {
        log::info!("  Color:    rtsp://localhost:{rtsp_port}/color");
        log::info!("  Infrared: rtsp://localhost:{rtsp_port}/infrared");
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
