use std::{sync::Arc, time::Duration};

use anyhow::Context;
// no async ring buffers needed for RTSP publishing path
use kinect_v2::infrared_capture::{
    InfraredFrameCapture, InfraredFrameCaptureIter, InfraredFrameData,
};
use ringbuf::{
    HeapRb, SharedRb,
    storage::Heap,
    traits::{Consumer, Producer, Split},
    wrap::caching::Caching,
};

use crate::infrared_config::{InfraredConfig, InfraredConfigManager};
use crate::rtsp_publisher::RtspPublisher;

/// InfraredSourceValueMaximum is the highest value that can be returned in the InfraredFrame.
/// It is cast to a float for readability in the visualization code.
const INFRARED_SOURCE_VALUE_MAXIMUM: f32 = u16::MAX as f32; // 65535.0

fn generate_lut(config: &InfraredConfig) -> [u8; 65536] {
    let mut lut = [0u8; 65536];
    for (infrared_point, grey_scale_pixel_byte) in lut.iter_mut().enumerate() {
        // Since we are displaying the image as a normalized grey scale image, we need to convert from
        // the u16 data (as provided by the InfraredFrame) to a value from [InfraredOutputValueMinimum, InfraredOutputValueMaximum]
        // Normalize → clamp → byte conversion:
        let f = (infrared_point as f32 / INFRARED_SOURCE_VALUE_MAXIMUM * config.infrared_source_scale)
            * (1.0 - config.infrared_output_value_minimum)
            + config.infrared_output_value_minimum;
        let clamped = config.infrared_output_value_maximum.min(f);
        *grey_scale_pixel_byte = (clamped * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    lut
}

fn configs_equal(a: &InfraredConfig, b: &InfraredConfig) -> bool {
    (a.infrared_output_value_minimum - b.infrared_output_value_minimum).abs() < f32::EPSILON
        && (a.infrared_output_value_maximum - b.infrared_output_value_maximum).abs() < f32::EPSILON
        && (a.infrared_source_scale - b.infrared_source_scale).abs() < f32::EPSILON
}

fn infrared_frame_capture(
    rtsp: Arc<RtspPublisher>,
    raw_tx: &mut Caching<Arc<SharedRb<Heap<InfraredFrameData>>>, true, false>,
) -> anyhow::Result<()> {
    let infrared_capture =
        InfraredFrameCapture::new().context("Failed to create infrared capture")?;

    let mut iter: Option<InfraredFrameCaptureIter> = None;

    let mut frame_count = 0;
    let mut last_log_time = std::time::Instant::now();

    loop {
        if !rtsp.is_infra_active() {
            log::debug!("RTSP infrared capture not active, skipping infrared capture");
            if iter.is_some() {
                // If we have an iter, drop it.
                iter = None;
            }

            // Sleep briefly to avoid busy waiting
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }

        if iter.is_none() {
            log::info!("Kinect infrared capture starting...");
            iter = Some(
                infrared_capture
                    .iter()
                    .context("Failed to create infrared capture iterator")?,
            );
        }

        if let Some(iter) = &mut iter {
            match iter.next() {
                Some(Ok(data)) => {
                    frame_count += 1;

                    // Log frame capture every 30 frames (approximately once per second at 30fps)
                    if frame_count % 30 == 0 || last_log_time.elapsed() > Duration::from_secs(5) {
                        log::debug!(
                            "✅ Captured infrared frame #{}: {}x{}",
                            frame_count,
                            data.width,
                            data.height
                        );
                        last_log_time = std::time::Instant::now();
                    }

                    if raw_tx.try_push(data).is_err() {
                        log::error!("❌ Infrared frame buffer full, dropping frame");
                    }
                }
                Some(Err(e)) => {
                    log::warn!("⚠️ Error capturing infrared frame: {e}");
                }
                None => {
                    // No new frame available yet - log periodically to show we're still trying
                    if last_log_time.elapsed() > Duration::from_secs(10) {
                        log::warn!(
                            "🔍 No infrared frames available from Kinect - is the device connected?"
                        );
                        last_log_time = std::time::Instant::now();
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
}

fn infrared_frame_publish(
    rtsp: Arc<RtspPublisher>,
    raw_rx: &mut Caching<Arc<SharedRb<Heap<InfraredFrameData>>>, false, true>,
    config_manager: Arc<InfraredConfigManager>,
) -> anyhow::Result<()> {
    // For dynamic LUT generation, we'll create a new one whenever config changes
    let mut current_config = config_manager.get_config();
    let mut lut = generate_lut(&current_config);
    let mut last_config_check = std::time::Instant::now();

    // pre‐allocate a single RGBA buffer. Kinect is always the same resolution,
    // so after the first frame we never re‐reserve.
    let mut rgba_data = Vec::new();
    let mut first_frame = true;

    loop {
        // Check for config changes every second
        if last_config_check.elapsed() > Duration::from_secs(1) {
            let new_config = config_manager.get_config();
            if !configs_equal(&current_config, &new_config) {
                log::info!("🔄 Regenerating infrared LUT with new config values");
                current_config = new_config;
                lut = generate_lut(&current_config);
            }
            last_config_check = std::time::Instant::now();
        }

        if let Some(infrared_frame) = raw_rx.try_pop() {
            if infrared_frame.data.is_empty() {
                log::debug!("Skipping empty infrared frame");
                continue; // Skip empty frames
            }

            // on first real frame, reserve full capacity
            if first_frame {
                let cap = (infrared_frame.width * infrared_frame.height * 4) as usize;
                rgba_data.reserve_exact(cap);
                first_frame = false;
            }

            // Convert infrared data to RGBA using the LUT and push to RTSP
            rgba_data.clear();
            for &pt in infrared_frame.data.iter() {
                let i = lut[pt as usize];
                rgba_data.extend_from_slice(&[i, i, i, 255]);
            }
            rtsp.send_infra_bgra(infrared_frame.width, infrared_frame.height, &rgba_data);
        } else {
            // No frame is available, sleep briefly to avoid busy waiting
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

pub fn spawn_infra_pipeline(rtsp: Arc<RtspPublisher>, config_manager: Arc<InfraredConfigManager>) {
    let raw_ring_buffer = HeapRb::<InfraredFrameData>::new(32);
    let (mut raw_tx, mut raw_rx) = raw_ring_buffer.split();

    let rtsp_clone = rtsp.clone();
    // Infrared frame capture thread
    std::thread::spawn(move || {
        if let Err(e) = infrared_frame_capture(rtsp_clone, &mut raw_tx) {
            log::error!("Error capturing infrared frames: {e}");
        }
    });

    // Infrared frame publish thread
    std::thread::spawn(move || {
        if let Err(e) = infrared_frame_publish(rtsp, &mut raw_rx, config_manager) {
            log::error!("Error publishing infrared frames: {e}");
        }
    });
}
