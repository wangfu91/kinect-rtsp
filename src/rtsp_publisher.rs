use anyhow::Result;
use glib::MainLoop;
use gstreamer::prelude::*;
use gstreamer::{self as gst, FlowError};
use gstreamer_app as gst_app;
use gstreamer_rtsp_server as rtsp;
use gstreamer_rtsp_server::prelude::*;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

// Store desired credentials when auth is enabled
static AUTH_CREDENTIALS: OnceCell<(String, String)> = OnceCell::new();

/// Simple RTSP Publisher based on GStreamer examples
/// Exposes two RTSP mount points:
/// - rtsp://<host>:port/color     (H.264 video + AAC audio)
/// - rtsp://<host>:port/infrared  (H.264 video + AAC audio)
pub struct RtspPublisher {
    color_src: Arc<Mutex<Option<gst_app::AppSrc>>>,
    color_audio_src: Arc<Mutex<Option<gst_app::AppSrc>>>,
    infra_src: Arc<Mutex<Option<gst_app::AppSrc>>>,
    infra_audio_src: Arc<Mutex<Option<gst_app::AppSrc>>>,
    color_client_count: Arc<AtomicUsize>,
    infra_client_count: Arc<AtomicUsize>,
    audio_conversion_buf: Arc<Mutex<Vec<i16>>>,
}

/// Checks if a GStreamer element is available, returning a detailed error if not.
fn check_gst_element(name: &str) -> Result<()> {
    if gst::ElementFactory::find(name).is_some() {
        log::info!("✅ GStreamer element found: {name}");
        Ok(())
    } else {
        let err_msg = format!(
            "Missing GStreamer element '{name}'. Please ensure GStreamer and the required plugins \
            (e.g., gst-plugins-good, gst-plugins-ugly, gst-plugins-bad) are installed correctly and accessible in your system's PATH."
        );
        log::error!("{err_msg}");
        Err(anyhow::anyhow!(err_msg))
    }
}

/// Helper to create and configure a factory for a stream (color or infrared).
fn create_factory(
    video_caps: &str,
    audio_caps: &str,
    video_bitrate: u32,
    audio_bitrate: u32,
    src_name: &str,
    audio_src_name: &str,
    max_video_bytes: u64,
    client_count: Arc<AtomicUsize>,
    video_src: Arc<Mutex<Option<gst_app::AppSrc>>>,
    audio_src: Arc<Mutex<Option<gst_app::AppSrc>>>,
) -> rtsp::RTSPMediaFactory {
    let factory = rtsp::RTSPMediaFactory::new();

    let video_pipeline = format!(
        "( appsrc name={src_name} is-live=true format=time do-timestamp=true \
        caps={video_caps} \
        ! queue leaky=downstream max-size-buffers=1 max-size-bytes=0 max-size-time=0 \
        ! videoconvert ! video/x-raw,format=I420 \
        ! queue leaky=downstream max-size-buffers=1 max-size-bytes=0 max-size-time=0 \
        ! openh264enc bitrate={video_bitrate} gop-size=30 complexity=low \
        ! h264parse config-interval=1 \
        ! rtph264pay name=pay0 pt=96 )"
    );

    let audio_pipeline = format!(
        "( appsrc name={audio_src_name} is-live=true format=time do-timestamp=true \
        caps={audio_caps} \
        ! queue leaky=downstream max-size-buffers=4 max-size-bytes=0 max-size-time=0 \
        ! audioconvert ! audioresample \
        ! avenc_aac bitrate={audio_bitrate} \
        ! rtpmp4apay name=pay1 pt=97 )"
    );

    let full_pipeline = format!("{video_pipeline}{audio_pipeline}");
    factory.set_launch(&full_pipeline);
    factory.set_shared(true);

    let video_src_clone = video_src.clone();
    let audio_src_clone = audio_src.clone();
    let count = client_count.clone();
    let src_name = src_name.to_string();
    let audio_src_name = audio_src_name.to_string();

    factory.connect_media_configure(move |_, media| {
        let active = count.fetch_add(1, Ordering::SeqCst) + 1;
        log::info!("🎥 /{src_name} session started, count = {active}");

        let count_inner = count.clone();
        let video_src_unprep = video_src_clone.clone();
        let audio_src_unprep = audio_src_clone.clone();
        let src_name_clone = src_name.clone();

        media.connect_unprepared(move |_| {
            let active = count_inner.fetch_sub(1, Ordering::SeqCst) - 1;
            log::info!("🎥 /{src_name_clone} session ended, count = {active}");
            *video_src_unprep.lock() = None;
            *audio_src_unprep.lock() = None;
        });

        let elem = media.element();
        if let Ok(bin) = elem.downcast::<gst::Bin>() {
            if let Some(src_elem) = bin.by_name(&src_name)
                && let Ok(appsrc) = src_elem.downcast::<gst_app::AppSrc>()
            {
                appsrc.set_format(gst::Format::Time);
                appsrc.set_block(true);
                appsrc.set_max_bytes(max_video_bytes);
                *video_src_clone.lock() = Some(appsrc);
                log::info!(
                    "{src_name} appsrc configured (block=true, max-bytes={max_video_bytes})"
                );
            }
            if let Some(audio_src_elem) = bin.by_name(&audio_src_name)
                && let Ok(appsrc) = audio_src_elem.downcast::<gst_app::AppSrc>()
            {
                appsrc.set_format(gst::Format::Time);
                appsrc.set_block(true);
                appsrc.set_max_bytes(512 * 1024);
                *audio_src_clone.lock() = Some(appsrc);
                log::info!("{audio_src_name} appsrc configured (block=true, max-bytes=512KB)");
            }
        }
    });

    factory
}

impl RtspPublisher {
    /// Returns true if color capture should be active (i.e., at least one client is connected to /color)
    pub fn is_color_active(&self) -> bool {
        self.color_client_count.load(Ordering::SeqCst) > 0
    }

    /// Returns true if infrared capture should be active (i.e., at least one client is connected to /infrared)
    pub fn is_infra_active(&self) -> bool {
        self.infra_client_count.load(Ordering::SeqCst) > 0
    }

    /// Returns true if any capture should be active
    pub fn is_capture_active(&self) -> bool {
        self.is_color_active() || self.is_infra_active()
    }

    pub fn start(username: Option<&str>, password: Option<&str>, port: u16) -> Result<Arc<Self>> {
        // Initialize GStreamer
        gst::init()?;

        // Check that all required GStreamer elements are available
        log::info!("Checking for required GStreamer elements...");
        check_gst_element("appsrc")?;
        check_gst_element("videoconvert")?;
        check_gst_element("openh264enc")?;
        check_gst_element("h264parse")?;
        check_gst_element("rtph264pay")?;
        // We'll use queue elements to bound buffering and drop under pressure
        check_gst_element("queue")?;
        // Checks for your audio branch:
        check_gst_element("audioresample")?;
        check_gst_element("audioconvert")?;
        check_gst_element("avenc_aac")?;
        check_gst_element("rtpmp4apay")?;
        log::info!("✅ All required GStreamer elements are available.");

        let main_loop = MainLoop::new(None, false);
        let server = rtsp::RTSPServer::new();

        // Optional Basic Auth (username/password). If both are provided, enable auth.
        if let (Some(user), Some(pass)) = (username, password) {
            AUTH_CREDENTIALS
                .set((user.to_string(), pass.to_string()))
                .ok();
            let auth = auth::Auth::default();
            server.set_auth(Some(&auth));
            log::info!("RTSP Basic Auth enabled for user '{user}'");
        } else {
            log::info!("RTSP Basic Auth disabled (no credentials provided)");
        }

        // Create per-mount-point client counters
        let color_client_count = Arc::new(AtomicUsize::new(0));
        let infra_client_count = Arc::new(AtomicUsize::new(0));

        // Set the port explicitly
        server.set_service(&port.to_string());

        // Get mount points
        let mounts = server.mount_points().expect("Failed to get mount points");

        // Shared appsrc handles
        let color_src: Arc<Mutex<Option<gst_app::AppSrc>>> = Arc::new(Mutex::new(None));
        let color_audio_src: Arc<Mutex<Option<gst_app::AppSrc>>> = Arc::new(Mutex::new(None));
        let infra_src: Arc<Mutex<Option<gst_app::AppSrc>>> = Arc::new(Mutex::new(None));
        let infra_audio_src: Arc<Mutex<Option<gst_app::AppSrc>>> = Arc::new(Mutex::new(None));

        // Color factory
        let color_factory = create_factory(
            "video/x-raw,format=BGRA,width=1920,height=1080,framerate=30/1",
            "audio/x-raw,format=S16LE,layout=interleaved,rate=16000,channels=1",
            2500000,
            128000,
            "colorsrc",
            "audiosrc",
            16 * 1024 * 1024,
            color_client_count.clone(),
            color_src.clone(),
            color_audio_src.clone(),
        );
        mounts.add_factory("/color", color_factory);

        // Infrared factory
        let infra_factory = create_factory(
            "video/x-raw,format=BGRA,width=512,height=424,framerate=30/1",
            "audio/x-raw,format=S16LE,layout=interleaved,rate=16000,channels=1",
            1500000,
            128000,
            "infrasrc",
            "infraaudiosrc",
            4 * 1024 * 1024,
            infra_client_count.clone(),
            infra_src.clone(),
            infra_audio_src.clone(),
        );
        mounts.add_factory("/infrared", infra_factory);

        // Attach server to main context - this is critical!
        let _id = server.attach(None).expect("Failed to attach RTSP server");

        // Additional server configuration
        server.set_address("0.0.0.0");
        log::info!("RTSP server configured on {:?}", server.address());

        log::info!("RTSP server ready at rtsp://127.0.0.1:{}/color", port);
        log::info!("RTSP server ready at rtsp://127.0.0.1:{}/infrared", port);
        log::info!("RTSP server ready at rtsp://localhost:{}/color", port);
        log::info!("RTSP server ready at rtsp://localhost:{}/infrared", port);
        log::info!("VLC: Open Media > Network Stream > Enter URL > Click Play");

        // Start the main loop in a background thread
        std::thread::spawn(move || {
            log::info!("Starting RTSP server main loop");
            main_loop.run();
        });

        Ok(Arc::new(Self {
            color_src,
            color_audio_src,
            infra_src,
            infra_audio_src,
            color_client_count,
            infra_client_count,
            audio_conversion_buf: Arc::new(Mutex::new(Vec::with_capacity(2048))),
        }))
    }

    pub fn send_color_bgra(&self, _width: u32, _height: u32, data: &[u8]) {
        if let Some(appsrc) = self.color_src.lock().as_ref() {
            let mut buffer = gst::Buffer::with_size(data.len()).expect("Failed to alloc GstBuffer");
            if let Ok(mut map) = buffer.get_mut().unwrap().map_writable() {
                map.copy_from_slice(data);
            }
            if let Err(e) = appsrc.push_buffer(buffer) {
                if e == FlowError::Flushing {
                    log::debug!("Color appsrc is flushing, ignoring push error");
                } else {
                    log::warn!("Failed to push color buffer: {e:?}");
                }
            }
        }
    }

    pub fn send_infra_bgra(&self, _width: u32, _height: u32, data: &[u8]) {
        if let Some(appsrc) = self.infra_src.lock().as_ref() {
            let mut buffer = gst::Buffer::with_size(data.len()).expect("Failed to alloc GstBuffer");
            if let Ok(mut map) = buffer.get_mut().unwrap().map_writable() {
                map.copy_from_slice(data);
            }
            if let Err(e) = appsrc.push_buffer(buffer) {
                if e == FlowError::Flushing {
                    log::debug!("Infrared appsrc is flushing, ignoring push error");
                } else {
                    log::warn!("Failed to push infrared buffer: {e:?}");
                }
            }
        }
    }

    pub fn send_audio_f32(&self, samples_f32: &[f32]) {
        // Reuse buffer to avoid allocation
        let mut s16_data = self.audio_conversion_buf.lock();
        s16_data.clear();
        s16_data.extend(
            samples_f32
                .iter()
                .map(|&sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16),
        );

        let bytes: &[u8] = bytemuck::cast_slice(&*s16_data);

        // Allocate a GStreamer buffer and copy, the bytes in; this avoids alignment pitfalls.
        let mut buffer = gst::Buffer::with_size(bytes.len()).expect("Failed to alloc GstBuffer");
        if let Ok(mut map) = buffer.get_mut().unwrap().map_writable() {
            map.copy_from_slice(bytes);
        }

        // Push to color audio stream
        if let Some(appsrc) = self.color_audio_src.lock().as_ref()
            && let Err(e) = appsrc.push_buffer(buffer.clone())
        {
            if e == FlowError::Flushing {
                log::debug!("Color audio appsrc is flushing, ignoring push error");
            } else {
                log::warn!("Failed to push color audio buffer: {e:?}");
            }
        }

        // Push to infrared audio stream
        if let Some(appsrc) = self.infra_audio_src.lock().as_ref()
            && let Err(e) = appsrc.push_buffer(buffer)
        {
            if e == FlowError::Flushing {
                log::debug!("Infrared audio appsrc is flushing, ignoring push error");
            } else {
                log::warn!("Failed to push infrared audio buffer: {e:?}");
            }
        }
    }
}

// Minimal custom RTSP auth module adapted from gstreamer-rs example, but validates
// against the optional credentials provided to RtspPublisher::start.
mod auth {
    mod imp {
        use super::super::AUTH_CREDENTIALS;
        use base64::Engine;
        use gstreamer_rtsp_server::gst_rtsp::{RTSPHeaderField, RTSPStatusCode};
        use gstreamer_rtsp_server::{RTSPContext, RTSPToken, prelude::*, subclass::prelude::*};

        #[derive(Default)]
        pub struct Auth;

        impl Auth {
            fn validate_basic(&self, authorization: &str) -> Option<String> {
                // Expect "Basic base64(user:pass)" but framework already gives the base64 payload
                // in the example via authorization(). Here we assume it's the base64 payload.
                // However, gst crate provides the raw auth string (base64). We'll decode and compare.
                if let Some((u, p)) = AUTH_CREDENTIALS.get()
                    && let Ok(decoded) =
                        base64::engine::general_purpose::STANDARD.decode(authorization.as_bytes())
                    && let Ok(decoded) = std::str::from_utf8(&decoded)
                {
                    let mut it = decoded.splitn(2, ':');
                    if let (Some(user), Some(pass)) = (it.next(), it.next())
                        && user == u
                        && pass == p
                    {
                        return Some(user.to_string());
                    }
                }
                None
            }
        }

        #[glib::object_subclass]
        impl ObjectSubclass for Auth {
            const NAME: &'static str = "RsRTSPAuthBasic";
            type Type = super::Auth;
            type ParentType = gstreamer_rtsp_server::RTSPAuth;
        }

        impl ObjectImpl for Auth {}

        impl RTSPAuthImpl for Auth {
            fn authenticate(&self, ctx: &RTSPContext) -> bool {
                let req = match ctx.request() {
                    Some(r) => r,
                    None => return false,
                };

                if let Some(auth_credentials) = req.parse_auth_credentials().first()
                    && let Some(authorization) = auth_credentials.authorization()
                    && let Some(user) = self.validate_basic(authorization)
                {
                    ctx.set_token(RTSPToken::builder().field("user", user).build());
                    return true;
                }
                false
            }

            fn check(&self, ctx: &RTSPContext, role: &glib::GString) -> bool {
                // Only guard factory access
                if !role.starts_with("auth.check.media.factory") {
                    return true;
                }

                // Ensure authenticated
                if ctx.token().is_none() && !self.authenticate(ctx) {
                    if let Some(resp) = ctx.response() {
                        resp.init_response(RTSPStatusCode::Unauthorized, ctx.request());
                        resp.add_header(
                            RTSPHeaderField::WwwAuthenticate,
                            "Basic realm=\"KinectRTSP\"",
                        );
                        if let Some(client) = ctx.client() {
                            client.send_message(resp, ctx.session());
                        }
                    }
                    return false;
                }

                true
            }
        }
    }

    glib::wrapper! {
        pub struct Auth(ObjectSubclass<imp::Auth>) @extends gstreamer_rtsp_server::RTSPAuth;
    }

    impl Default for Auth {
        fn default() -> Self {
            glib::Object::new()
        }
    }
}
