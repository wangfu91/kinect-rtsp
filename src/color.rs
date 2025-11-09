use std::{sync::Arc, time::Duration};

use anyhow::Context;
use kinect_v2::{
    ColorImageFormat,
    color_capture::{ColorFrameCapture, ColorFrameCaptureIter, ColorFrameData},
};
use ringbuf::{
    HeapRb, SharedRb,
    storage::Heap,
    traits::{Consumer, Producer, Split},
    wrap::caching::Caching,
};

use crate::rtsp_publisher::RtspPublisher;

fn color_frame_capture(
    rtsp: Arc<RtspPublisher>,
    raw_tx: &mut Caching<Arc<SharedRb<Heap<ColorFrameData>>>, true, false>,
) -> anyhow::Result<()> {
    let mut color_capture: Option<ColorFrameCapture> = None;
    let mut iter: Option<ColorFrameCaptureIter> = None;

    let mut frame_count = 0;
    let mut last_log_time = std::time::Instant::now();

    loop {
        if !rtsp.is_color_active() {
            // RTSP color capture not active, release Kinect resources.
            if iter.is_some() {
                iter = None;
                log::info!("Kinect color capture paused (no active subscribers)");
            }
            if color_capture.take().is_some() {
                log::debug!("Kinect color capture resources released");
            }
            std::thread::sleep(Duration::from_millis(30));
            continue;
        }

        if iter.is_none() {
            if color_capture.is_none() {
                log::info!("Kinect color capture starting...");
                color_capture = Some(
                    ColorFrameCapture::new_with_format(ColorImageFormat::Yuy2)
                        .context("Failed to create color capture with YUY2 format")?,
                );
            }

            if let Some(capture) = color_capture.as_ref() {
                iter = Some(
                    capture
                        .iter()
                        .context("Failed to create color capture iterator")?,
                );
            } else {
                std::thread::sleep(Duration::from_millis(30));
                continue;
            }
        }

        if let Some(iter) = &mut iter {
            match iter.next() {
                Some(Ok(data)) => {
                    frame_count += 1;
                    if frame_count % 30 == 0 || last_log_time.elapsed() > Duration::from_secs(5) {
                        log::debug!(
                            "✅ Captured color frame #{}: {}x{}",
                            frame_count,
                            data.width,
                            data.height
                        );
                        last_log_time = std::time::Instant::now();
                    }
                    if raw_tx.try_push(data).is_err() {
                        log::debug!("❌ Color frame buffer full, dropping frame");
                    }
                }
                Some(Err(e)) => {
                    log::warn!("⚠️ Error capturing color frame: {e}");
                }
                None => {
                    if last_log_time.elapsed() > Duration::from_secs(10) {
                        log::warn!(
                            "🔍 No color frames available from Kinect - is the device connected?"
                        );
                        last_log_time = std::time::Instant::now();
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
}

fn color_frame_publish(
    rtsp: Arc<RtspPublisher>,
    raw_rx: &mut Caching<Arc<SharedRb<Heap<ColorFrameData>>>, false, true>,
) -> anyhow::Result<()> {
    loop {
        if let Some(color_frame) = raw_rx.try_pop() {
            if color_frame.data.is_empty() {
                continue;
            }
            assert_eq!(
                color_frame.image_format,
                ColorImageFormat::Yuy2,
                "Color frame format mismatch"
            );

            rtsp.send_color_yuy2(color_frame.width, color_frame.height, &color_frame.data);
        } else {
            // No new frame yet, sleep briefly to avoid busy waiting
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}

pub fn spawn_color_pipeline(rtsp: Arc<RtspPublisher>) {
    // Limit buffering to reduce peak memory: 16 x 1920x1080 YUY2 ~ 64MB
    let raw_ring_buffer = HeapRb::<ColorFrameData>::new(16);
    let (mut raw_tx, mut raw_rx) = raw_ring_buffer.split();

    let rtsp_clone = rtsp.clone();
    // Color capture thread
    std::thread::spawn(move || {
        if let Err(e) = color_frame_capture(rtsp_clone, &mut raw_tx) {
            log::error!("Error capturing color frames: {e}");
        }
    });

    // Publish thread
    std::thread::spawn(move || {
        if let Err(e) = color_frame_publish(rtsp, &mut raw_rx) {
            log::error!("Error publishing color frames: {e}");
        }
    });
}
