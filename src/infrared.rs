use std::{sync::Arc, time::Duration};

use anyhow::Context;
// no async ring buffers needed for RTSP publishing path
use kinect_v2::infrared_capture::{
    InfraredFrameCapture, InfraredFrameCaptureIter, InfraredFrameData,
};
use once_cell::sync::Lazy;
use ringbuf::{
    HeapRb, SharedRb,
    storage::Heap,
    traits::{Consumer, Producer, Split},
    wrap::caching::Caching,
};

use crate::rtsp_publisher::RtspPublisher;

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
) -> anyhow::Result<()> {
    /// InfraredSourceValueMaximum is the highest value that can be returned in the InfraredFrame.
    /// It is cast to a float for readability in the visualization code.
    const INFRARED_SOURCE_VALUE_MAXIMUM: f32 = u16::MAX as f32; // 65535.0

    /// The InfraredOutputValueMinimum value is used to set the lower limit, post processing, of the
    /// infrared data that we will render.
    /// Increasing or decreasing this value sets a brightness "wall" either closer or further away.
    const INFRARED_OUTPUT_VALUE_MINIMUM: f32 = 0.0;

    /// The InfraredOutputValueMaximum value is the upper limit, post processing, of the
    /// infrared data that we will render.
    const INFRARED_OUTPUT_VALUE_MAXIMUM: f32 = 1.0;

    /// The value by which the infrared source data will be scaled.
    const INFRARED_SOURCE_SCALE: f32 = 1.68;

    // Build a 64 KiB Lookup Table (LUT) once.
    // • once_cell::sync::Lazy ensures that closure runs exactly once (the first time you reference LUT), in a thread-safe way.
    // • After that, every pixel becomes just an index into that 64 KiB table, which is orders of magnitude faster than doing the full float pipeline per pixel.
    static LUT: Lazy<[u8; 65536]> = Lazy::new(|| {
        let mut lut = [0u8; 65536];
        for (infrared_point, grey_scale_pixel_byte) in lut.iter_mut().enumerate() {
            // Since we are displaying the image as a normalized grey scale image, we need to convert from
            // the u16 data (as provided by the InfraredFrame) to a value from [InfraredOutputValueMinimum, InfraredOutputValueMaximum]
            // Normalize → clamp → byte conversion:
            let f = (infrared_point as f32 / INFRARED_SOURCE_VALUE_MAXIMUM * INFRARED_SOURCE_SCALE)
                * (1.0 - INFRARED_OUTPUT_VALUE_MINIMUM)
                + INFRARED_OUTPUT_VALUE_MINIMUM;
            let clamped = INFRARED_OUTPUT_VALUE_MAXIMUM.min(f);
            *grey_scale_pixel_byte = (clamped * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        lut
    });

    // pre‐allocate a single RGBA buffer. Kinect is always the same resolution,
    // so after the first frame we never re‐reserve.
    let mut rgba_data = Vec::new();
    let mut first_frame = true;

    loop {
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
                let i = LUT[pt as usize];
                rgba_data.extend_from_slice(&[i, i, i, 255]);
            }
            rtsp.send_infra_bgra(infrared_frame.width, infrared_frame.height, &rgba_data);
        } else {
            // No frame is available, sleep briefly to avoid busy waiting
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

pub fn spawn_infra_pipeline(rtsp: Arc<RtspPublisher>) {
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
        if let Err(e) = infrared_frame_publish(rtsp, &mut raw_rx) {
            log::error!("Error publishing infrared frames: {e}");
        }
    });
}
