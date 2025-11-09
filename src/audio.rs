use std::{sync::Arc, time::Duration};

use anyhow::Context;
use bytemuck::try_cast_slice;
// no async ring buffers needed for RTSP publishing path
use kinect_v2::audio_capture::{AudioFrameCapture, AudioFrameCaptureIter, AudioFrameData};
use ringbuf::{
    HeapRb, SharedRb,
    storage::Heap,
    traits::{Consumer, Producer, Split},
    wrap::caching::Caching,
};

use crate::audio_frame_buffer::AudioFrameBuffer;
use crate::rtsp_publisher::RtspPublisher;

fn audio_frame_capture(
    rtsp: Arc<RtspPublisher>,
    raw_tx: &mut Caching<Arc<SharedRb<Heap<AudioFrameData>>>, true, false>,
) -> anyhow::Result<()> {
    let mut audio_capture: Option<AudioFrameCapture> = None;
    let mut iter: Option<AudioFrameCaptureIter> = None;

    let mut frame_count = 0;
    let mut last_log_time = std::time::Instant::now();

    loop {
        if !rtsp.is_capture_active() {
            // RTSP capture not active, release Kinect resources.
            if iter.is_some() {
                iter = None;
                log::info!("Kinect audio capture paused (no active subscribers)");
            }
            if audio_capture.take().is_some() {
                log::debug!("Kinect audio capture resources released");
            }
            std::thread::sleep(Duration::from_millis(30));
            continue;
        }

        if iter.is_none() {
            if audio_capture.is_none() {
                log::info!("Kinect audio capture starting...");
                audio_capture =
                    Some(AudioFrameCapture::new().context("Failed to create audio capture")?);
            }

            if let Some(capture) = audio_capture.as_ref() {
                iter = Some(
                    capture
                        .iter()
                        .context("Failed to create audio capture iterator")?,
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

                    // Log audio capture every 100 frames (less frequent than video)
                    if frame_count % 100 == 0 || last_log_time.elapsed() > Duration::from_secs(10) {
                        log::debug!("🎵 Captured audio frame #{frame_count}");
                        last_log_time = std::time::Instant::now();
                    }

                    if raw_tx.try_push(data).is_err() {
                        log::debug!("❌ Audio frame ring buffer full, dropping frame");
                    }
                }
                Some(Err(e)) => {
                    log::warn!("⚠️ Error capturing audio frame: {e}");
                }
                None => {
                    // No new frame available yet - log periodically to show we're still trying
                    if last_log_time.elapsed() > Duration::from_secs(15) {
                        log::warn!(
                            "🔍 No audio frames available from Kinect - is the device connected?"
                        );
                        last_log_time = std::time::Instant::now();
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    }
}

fn audio_frame_publish(
    rtsp: Arc<RtspPublisher>,
    raw_rx: &mut Caching<Arc<SharedRb<Heap<AudioFrameData>>>, false, true>,
) -> anyhow::Result<()> {
    let mut audio_frame_buffer = AudioFrameBuffer::<f32>::new();
    // RTSP branch expects S16LE 16kHz mono; we’ll buffer in 20ms chunks (320 samples)
    const FRAME_SIZE: usize = 320;

    loop {
        if let Some(audio_frame) = raw_rx.try_pop() {
            if audio_frame.data.is_empty() {
                log::trace!("Skipping empty audio frame");
                continue;
            }

            // Decode raw bytes into f32 samples without per-frame allocation
            match try_cast_slice::<u8, f32>(&audio_frame.data) {
                Ok(samples) => {
                    audio_frame_buffer.append_samples(samples.iter().copied());
                }
                Err(err) => {
                    log::warn!(
                        "Unexpected audio frame layout ({} bytes): {err}",
                        audio_frame.data.len()
                    );
                    continue;
                }
            }

            // Process each full 320‐sample chunk by sending it to RTSP (it will be converted to S16 in publisher)
            while let Some(input_chunk) = audio_frame_buffer.pop_frame(FRAME_SIZE) {
                rtsp.send_audio_f32(&input_chunk);
            }
        } else {
            // No new frame yet, sleep briefly to avoid busy waiting
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}

pub fn spawn_audio_pipeline(rtsp: Arc<RtspPublisher>) {
    let raw_ring_buffer = HeapRb::<AudioFrameData>::new(32);
    let (mut raw_tx, mut raw_rx) = raw_ring_buffer.split();

    let rtsp_clone = rtsp.clone();
    // Audio capture thread
    std::thread::spawn(move || {
        if let Err(e) = audio_frame_capture(rtsp_clone, &mut raw_tx) {
            log::error!("Error capturing audio frames: {e}");
        }
    });

    // Audio publish thread
    std::thread::spawn(move || {
        if let Err(e) = audio_frame_publish(rtsp, &mut raw_rx) {
            log::error!("Error publishing audio frames: {e}");
        }
    });
}
