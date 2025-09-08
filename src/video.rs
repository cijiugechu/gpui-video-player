use crate::Error;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_app::prelude::*;
use gstreamer_video as gst_video;
// Note: GPUI imports removed since we're using simple Vec<u8> for RGBA data
use gst::message::MessageView;
use parking_lot::{Mutex, RwLock};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Position in the media.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Position {
    /// Position based on time.
    Time(Duration),
    /// Position based on nth frame.
    Frame(u64),
}

impl From<Position> for gst::GenericFormattedValue {
    fn from(pos: Position) -> Self {
        match pos {
            Position::Time(t) => gst::ClockTime::from_nseconds(t.as_nanos() as _).into(),
            Position::Frame(f) => gst::format::Default::from_u64(f).into(),
        }
    }
}

impl From<Duration> for Position {
    fn from(t: Duration) -> Self {
        Position::Time(t)
    }
}

impl From<u64> for Position {
    fn from(f: u64) -> Self {
        Position::Frame(f)
    }
}

#[derive(Debug)]
pub(crate) struct Frame(gst::Sample);

impl Frame {
    pub fn empty() -> Self {
        Self(gst::Sample::builder().build())
    }

    pub fn readable(&'_ self) -> Option<gst::BufferMap<'_, gst::buffer::Readable>> {
        self.0.buffer().and_then(|x| x.map_readable().ok())
    }
}

/// Options for initializing a `Video` without post-construction locking.
#[derive(Debug, Clone)]
pub struct VideoOptions {
    /// Optional initial frame buffer capacity (0 disables buffering). Defaults to 3.
    pub frame_buffer_capacity: Option<usize>,
    /// Optional initial looping flag. Defaults to false.
    pub looping: Option<bool>,
    /// Optional initial playback speed. Defaults to 1.0.
    pub speed: Option<f64>,
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            frame_buffer_capacity: Some(3),
            looping: Some(false),
            speed: Some(1.0),
        }
    }
}

#[derive(Debug)]
#[allow(unused)]
pub(crate) struct Internal {
    pub(crate) id: u64,
    pub(crate) bus: gst::Bus,
    pub(crate) source: gst::Pipeline,
    pub(crate) alive: Arc<AtomicBool>,
    pub(crate) worker: Option<std::thread::JoinHandle<()>>,

    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) framerate: f64,
    pub(crate) duration: Duration,
    pub(crate) speed: f64,

    pub(crate) frame: Arc<Mutex<Frame>>,
    pub(crate) upload_frame: Arc<AtomicBool>,
    pub(crate) frame_buffer: Arc<Mutex<VecDeque<Frame>>>,
    pub(crate) frame_buffer_capacity: Arc<AtomicUsize>,
    pub(crate) last_frame_time: Arc<Mutex<Instant>>,
    pub(crate) looping: bool,
    pub(crate) is_eos: Arc<AtomicBool>,
    pub(crate) restart_stream: bool,

    pub(crate) subtitle_text: Arc<Mutex<Option<String>>>,
    pub(crate) upload_text: Arc<AtomicBool>,

    // Optional display size overrides. If only one is set, the other is
    // inferred using the natural aspect ratio (width / height).
    pub(crate) display_width_override: Option<u32>,
    pub(crate) display_height_override: Option<u32>,
}

impl Internal {
    pub(crate) fn seek(&self, position: impl Into<Position>, accurate: bool) -> Result<(), Error> {
        let position = position.into();

        match &position {
            Position::Time(_) => self.source.seek(
                self.speed,
                gst::SeekFlags::FLUSH
                    | if accurate {
                        gst::SeekFlags::ACCURATE
                    } else {
                        gst::SeekFlags::KEY_UNIT | gst::SeekFlags::SNAP_NEAREST
                    },
                gst::SeekType::Set,
                gst::GenericFormattedValue::from(position),
                gst::SeekType::Set,
                gst::ClockTime::NONE,
            )?,
            Position::Frame(_) => self.source.seek(
                self.speed,
                gst::SeekFlags::FLUSH
                    | if accurate {
                        gst::SeekFlags::ACCURATE
                    } else {
                        gst::SeekFlags::KEY_UNIT | gst::SeekFlags::SNAP_NEAREST
                    },
                gst::SeekType::Set,
                gst::GenericFormattedValue::from(position),
                gst::SeekType::Set,
                gst::format::Default::NONE,
            )?,
        };

        *self.subtitle_text.lock() = None;
        self.upload_text.store(true, Ordering::SeqCst);

        // Clear any buffered frames so old frames do not display after a seek,
        // which can visually appear as a larger-than-intended jump.
        self.frame_buffer.lock().clear();
        self.upload_frame.store(false, Ordering::SeqCst);

        Ok(())
    }

    pub(crate) fn set_speed(&mut self, speed: f64) -> Result<(), Error> {
        let Some(position) = self.source.query_position::<gst::ClockTime>() else {
            return Err(Error::Caps);
        };
        if speed > 0.0 {
            self.source.seek(
                speed,
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                gst::SeekType::Set,
                position,
                gst::SeekType::End,
                gst::ClockTime::from_seconds(0),
            )?;
        } else {
            self.source.seek(
                speed,
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                gst::SeekType::Set,
                gst::ClockTime::from_seconds(0),
                gst::SeekType::Set,
                position,
            )?;
        }
        self.speed = speed;
        Ok(())
    }

    pub(crate) fn restart_stream(&mut self) -> Result<(), Error> {
        self.is_eos.store(false, Ordering::SeqCst);
        self.set_paused(false);
        self.seek(0, false)?;
        Ok(())
    }

    pub(crate) fn set_paused(&mut self, paused: bool) {
        self.source
            .set_state(if paused {
                gst::State::Paused
            } else {
                gst::State::Playing
            })
            .unwrap(/* state was changed in ctor; state errors caught there */);

        if self.is_eos.load(Ordering::Acquire) && !paused {
            self.restart_stream = true;
        }
    }

    pub(crate) fn paused(&self) -> bool {
        self.source.state(gst::ClockTime::ZERO).1 == gst::State::Paused
    }
}

/// A multimedia video loaded from a URI (e.g., a local file path or HTTP stream).
#[derive(Debug, Clone)]
pub struct Video(pub(crate) Arc<RwLock<Internal>>);

impl Drop for Video {
    fn drop(&mut self) {
        // Only cleanup if this is the last reference
        if Arc::strong_count(&self.0) == 1
            && let Some(mut inner) = self.0.try_write()
        {
            inner
                .source
                .set_state(gst::State::Null)
                .expect("failed to set state");

            inner.alive.store(false, Ordering::SeqCst);
            if let Some(worker) = inner.worker.take()
                && let Err(err) = worker.join()
            {
                match err.downcast_ref::<String>() {
                    Some(e) => log::error!("Video thread panicked: {e}"),
                    None => log::error!("Video thread panicked with unknown reason"),
                }
            }
        }
    }
}

impl Video {
    /// Create a new video player from a given video which loads from `uri`.
    pub fn new(uri: &url::Url) -> Result<Self, Error> {
        Self::new_with_options(uri, VideoOptions::default())
    }

    /// Create a new video player from a given video which loads from `uri`,
    /// applying initialization options.
    pub fn new_with_options(uri: &url::Url, options: VideoOptions) -> Result<Self, Error> {
        gst::init()?;

        let pipeline = format!(
            "playbin uri=\"{}\" video-sink=\"videoscale ! videoconvert ! appsink name=gpui_video drop=true max-buffers=3 enable-last-sample=false caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1\"",
            uri.as_str()
        );
        let pipeline = gst::parse::launch(pipeline.as_ref())?
            .downcast::<gst::Pipeline>()
            .map_err(|_| Error::Cast)?;

        let video_sink: gst::Element = pipeline.property("video-sink");
        let pad = video_sink.pads().first().cloned().unwrap();
        let pad = pad.dynamic_cast::<gst::GhostPad>().unwrap();
        let bin = pad
            .parent_element()
            .unwrap()
            .downcast::<gst::Bin>()
            .unwrap();
        let video_sink = bin.by_name("gpui_video").unwrap();
        let video_sink = video_sink.downcast::<gst_app::AppSink>().unwrap();

        Self::from_gst_pipeline_with_options(pipeline, video_sink, None, options)
    }

    /// Creates a new video based on an existing GStreamer pipeline and appsink.
    pub fn from_gst_pipeline(
        pipeline: gst::Pipeline,
        video_sink: gst_app::AppSink,
        text_sink: Option<gst_app::AppSink>,
    ) -> Result<Self, Error> {
        Self::from_gst_pipeline_with_options(
            pipeline,
            video_sink,
            text_sink,
            VideoOptions::default(),
        )
    }

    /// Creates a new video based on an existing GStreamer pipeline and appsink,
    /// applying initialization options.
    pub fn from_gst_pipeline_with_options(
        pipeline: gst::Pipeline,
        video_sink: gst_app::AppSink,
        text_sink: Option<gst_app::AppSink>,
        options: VideoOptions,
    ) -> Result<Self, Error> {
        gst::init()?;
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        macro_rules! cleanup {
            ($expr:expr) => {
                $expr.map_err(|e| {
                    let _ = pipeline.set_state(gst::State::Null);
                    e
                })
            };
        }

        // Configure sinks to prevent unbounded buffering in appsink
        // Keep a small queue in the sink to allow smooth pull_sample
        video_sink.set_drop(true);
        video_sink.set_max_buffers(3);
        video_sink.set_property("enable-last-sample", false);
        if let Some(ref ts) = text_sink {
            ts.set_drop(true);
            ts.set_max_buffers(1);
            ts.set_property("enable-last-sample", false);
        }

        let pad = video_sink.pads().first().cloned().unwrap();

        cleanup!(pipeline.set_state(gst::State::Playing))?;

        // Wait a brief moment for the pipeline to start playing
        let _ = pipeline.state(gst::ClockTime::from_mseconds(100));
        cleanup!(pipeline.state(gst::ClockTime::from_seconds(5)).0)?;

        let caps = cleanup!(pad.current_caps().ok_or(Error::Caps))?;
        let s = cleanup!(caps.structure(0).ok_or(Error::Caps))?;
        let width = cleanup!(s.get::<i32>("width").map_err(|_| Error::Caps))?;
        let height = cleanup!(s.get::<i32>("height").map_err(|_| Error::Caps))?;
        let framerate = cleanup!(s.get::<gst::Fraction>("framerate").map_err(|_| Error::Caps))?;
        let framerate = framerate.numer() as f64 / framerate.denom() as f64;

        // Obtain video info from caps for NV12 format
        let vinfo = cleanup!(gst_video::VideoInfo::from_caps(&caps).map_err(|_| Error::Caps))?;
        let _row_stride0 = vinfo.stride()[0] as usize;

        if framerate.is_nan()
            || framerate.is_infinite()
            || framerate < 0.0
            || framerate.abs() < f64::EPSILON
        {
            let _ = pipeline.set_state(gst::State::Null);
            return Err(Error::Framerate(framerate));
        }

        let duration = Duration::from_nanos(
            pipeline
                .query_duration::<gst::ClockTime>()
                .map(|duration| duration.nseconds())
                .unwrap_or(0),
        );

        let frame = Arc::new(Mutex::new(Frame::empty()));
        let upload_frame = Arc::new(AtomicBool::new(false));
        let frame_buffer = Arc::new(Mutex::new(VecDeque::new()));
        // Default to a small buffer so the element can consume buffered frames
        let frame_buffer_capacity = Arc::new(AtomicUsize::new(
            options.frame_buffer_capacity.unwrap_or_default(),
        ));
        let alive = Arc::new(AtomicBool::new(true));
        let last_frame_time = Arc::new(Mutex::new(Instant::now()));

        let frame_ref = Arc::clone(&frame);
        let upload_frame_ref = Arc::clone(&upload_frame);
        let frame_buffer_ref = Arc::clone(&frame_buffer);
        let frame_buffer_capacity_ref = Arc::clone(&frame_buffer_capacity);
        let alive_ref = Arc::clone(&alive);
        let last_frame_time_ref = Arc::clone(&last_frame_time);

        let subtitle_text = Arc::new(Mutex::new(None));
        let upload_text = Arc::new(AtomicBool::new(false));
        let subtitle_text_ref = Arc::clone(&subtitle_text);
        let upload_text_ref = Arc::clone(&upload_text);

        let pipeline_ref = pipeline.clone();
        let bus_ref = pipeline_ref.bus().unwrap();
        let is_eos = Arc::new(AtomicBool::new(false));
        let is_eos_ref = Arc::clone(&is_eos);

        let worker = std::thread::spawn(move || {
            let mut clear_subtitles_at = None;

            while alive_ref.load(Ordering::Acquire) {
                // Drain bus messages to detect EOS/errors
                while let Some(msg) = bus_ref.timed_pop(gst::ClockTime::from_seconds(0)) {
                    match msg.view() {
                        MessageView::Eos(_) => {
                            is_eos_ref.store(true, Ordering::SeqCst);
                        }
                        MessageView::Error(err) => {
                            let debug = err.debug().unwrap_or_default();
                            log::error!(
                                "gstreamer error from {:?}: {} ({debug})",
                                err.src(),
                                err.error()
                            );
                        }
                        _ => {}
                    }
                }

                if is_eos_ref.load(Ordering::Acquire) {
                    // Stop busy-polling once EOS reached
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
                if let Err(err) = (|| -> Result<(), gst::FlowError> {
                    // Try to pull a new sample; on timeout just continue (no frame this tick)
                    let maybe_sample =
                        if pipeline_ref.state(gst::ClockTime::ZERO).1 != gst::State::Playing {
                            video_sink.try_pull_preroll(gst::ClockTime::from_mseconds(16))
                        } else {
                            video_sink.try_pull_sample(gst::ClockTime::from_mseconds(16))
                        };

                    let Some(sample) = maybe_sample else {
                        // No sample available yet (timeout). Don't treat as error.
                        return Ok(());
                    };

                    *last_frame_time_ref.lock() = Instant::now();

                    let frame_segment = sample.segment().cloned().ok_or(gst::FlowError::Error)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    let frame_pts = buffer.pts().ok_or(gst::FlowError::Error)?;
                    let frame_duration = buffer.duration().ok_or(gst::FlowError::Error)?;

                    // Store the NV12 sample directly for GPU processing
                    {
                        let mut frame_guard = frame_ref.lock();
                        *frame_guard = Frame(sample);
                    }

                    // Push into frame buffer if enabled, trimming to capacity
                    let capacity = frame_buffer_capacity_ref.load(Ordering::SeqCst);
                    if capacity > 0 {
                        let sample_for_buffer = frame_ref.lock().0.clone();
                        let mut buf = frame_buffer_ref.lock();
                        buf.push_back(Frame(sample_for_buffer));
                        while buf.len() > capacity {
                            buf.pop_front();
                        }
                    }

                    // Always mark frame as ready for upload
                    upload_frame_ref.store(true, Ordering::SeqCst);

                    // Handle subtitles
                    if let Some(at) = clear_subtitles_at
                        && frame_pts >= at
                    {
                        *subtitle_text_ref.lock() = None;
                        upload_text_ref.store(true, Ordering::SeqCst);
                        clear_subtitles_at = None;
                    }

                    let text = text_sink
                        .as_ref()
                        .and_then(|sink| sink.try_pull_sample(gst::ClockTime::from_seconds(0)));
                    if let Some(text) = text {
                        let text_segment = text.segment().ok_or(gst::FlowError::Error)?;
                        let text = text.buffer().ok_or(gst::FlowError::Error)?;
                        let text_pts = text.pts().ok_or(gst::FlowError::Error)?;
                        let text_duration = text.duration().ok_or(gst::FlowError::Error)?;

                        let frame_running_time = frame_segment.to_running_time(frame_pts).value();
                        let frame_running_time_end = frame_segment
                            .to_running_time(frame_pts + frame_duration)
                            .value();

                        let text_running_time = text_segment.to_running_time(text_pts).value();
                        let text_running_time_end = text_segment
                            .to_running_time(text_pts + text_duration)
                            .value();

                        if text_running_time_end > frame_running_time
                            && frame_running_time_end > text_running_time
                        {
                            let duration = text.duration().unwrap_or(gst::ClockTime::ZERO);
                            let map = text.map_readable().map_err(|_| gst::FlowError::Error)?;

                            let text = std::str::from_utf8(map.as_slice())
                                .map_err(|_| gst::FlowError::Error)?
                                .to_string();
                            *subtitle_text_ref.lock() = Some(text);
                            upload_text_ref.store(true, Ordering::SeqCst);

                            clear_subtitles_at = Some(text_pts + duration);
                        }
                    }

                    Ok(())
                })() {
                    // Only log non-EOS errors
                    if err != gst::FlowError::Eos {
                        log::error!("error processing frame: {:?}", err);
                    }
                }
            }
        });

        // Apply initial playback speed if specified (must be after pipeline started)
        let initial_speed = options.speed.unwrap_or_default();
        if (initial_speed - 1.0).abs() > f64::EPSILON {
            let position = cleanup!(
                pipeline
                    .query_position::<gst::ClockTime>()
                    .ok_or(Error::Caps)
            )?;
            if initial_speed > 0.0 {
                cleanup!(pipeline.seek(
                    initial_speed,
                    gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                    gst::SeekType::Set,
                    position,
                    gst::SeekType::End,
                    gst::ClockTime::from_seconds(0),
                ))?;
            } else {
                cleanup!(pipeline.seek(
                    initial_speed,
                    gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                    gst::SeekType::Set,
                    gst::ClockTime::from_seconds(0),
                    gst::SeekType::Set,
                    position,
                ))?;
            }
        }

        Ok(Video(Arc::new(RwLock::new(Internal {
            id,
            bus: pipeline.bus().unwrap(),
            source: pipeline,
            alive,
            worker: Some(worker),

            width,
            height,
            framerate,
            duration,
            speed: initial_speed,

            frame,
            upload_frame,
            frame_buffer,
            frame_buffer_capacity,
            last_frame_time,
            looping: options.looping.unwrap_or_default(),
            is_eos,
            restart_stream: false,

            subtitle_text,
            upload_text,

            display_width_override: None,
            display_height_override: None,
        }))))
    }

    pub(crate) fn read(&'_ self) -> parking_lot::RwLockReadGuard<'_, Internal> {
        self.0.read()
    }

    pub(crate) fn write(&'_ self) -> parking_lot::RwLockWriteGuard<'_, Internal> {
        self.0.write()
    }

    /// Get the size/resolution of the video as `(width, height)`.
    pub fn size(&self) -> (i32, i32) {
        (self.read().width, self.read().height)
    }

    /// Get the natural aspect ratio (width / height) of the video as f32.
    pub fn aspect_ratio(&self) -> f32 {
        let (w, h) = self.size();
        if w <= 0 || h <= 0 {
            return 1.0;
        }
        w as f32 / h as f32
    }

    /// Set an override display width in pixels. Pass `None` to clear.
    pub fn set_display_width(&self, width: Option<u32>) {
        self.write().display_width_override = width;
    }

    /// Set an override display height in pixels. Pass `None` to clear.
    pub fn set_display_height(&self, height: Option<u32>) {
        self.write().display_height_override = height;
    }

    /// Set override display size in pixels. Any value set to `None` is cleared.
    pub fn set_display_size(&self, width: Option<u32>, height: Option<u32>) {
        let mut inner = self.write();
        inner.display_width_override = width;
        inner.display_height_override = height;
    }

    /// Get the effective display size honoring overrides. If only one of
    /// width/height is overridden, the other is inferred from the natural
    /// aspect ratio, rounded to nearest pixel.
    pub fn display_size(&self) -> (u32, u32) {
        let inner = self.read();
        let natural_w = inner.width.max(0) as u32;
        let natural_h = inner.height.max(0) as u32;
        let ar = if natural_h == 0 {
            1.0
        } else {
            natural_w as f32 / natural_h as f32
        };

        match (inner.display_width_override, inner.display_height_override) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => {
                let h = if ar == 0.0 {
                    natural_h
                } else {
                    (w as f32 / ar).round() as u32
                };
                (w, h)
            }
            (None, Some(h)) => {
                let w = ((h as f32) * ar).round() as u32;
                (w, h)
            }
            (None, None) => (natural_w, natural_h),
        }
    }

    /// Get the framerate of the video as frames per second.
    pub fn framerate(&self) -> f64 {
        self.read().framerate
    }

    /// Set the volume multiplier of the audio.
    pub fn set_volume(&self, volume: f64) {
        {
            let inner = self.write();
            inner.source.set_property("volume", volume);
        }
        let muted = self.muted();
        self.set_muted(muted);
    }

    /// Get the volume multiplier of the audio.
    pub fn volume(&self) -> f64 {
        self.read().source.property("volume")
    }

    /// Set if the audio is muted or not.
    pub fn set_muted(&self, muted: bool) {
        self.write().source.set_property("mute", muted);
    }

    /// Get if the audio is muted or not.
    pub fn muted(&self) -> bool {
        self.read().source.property("mute")
    }

    /// Get if the stream ended or not.
    pub fn eos(&self) -> bool {
        self.read().is_eos.load(Ordering::Acquire)
    }

    /// Get if the media will loop or not.
    pub fn looping(&self) -> bool {
        self.read().looping
    }

    /// Set if the media will loop or not.
    pub fn set_looping(&self, looping: bool) {
        self.write().looping = looping;
    }

    /// Set if the media is paused or not.
    pub fn set_paused(&self, paused: bool) {
        self.write().set_paused(paused)
    }

    /// Get if the media is paused or not.
    pub fn paused(&self) -> bool {
        self.read().paused()
    }

    /// Jumps to a specific position in the media.
    pub fn seek(&self, position: impl Into<Position>, accurate: bool) -> Result<(), Error> {
        self.write().seek(position, accurate)
    }

    /// Set the playback speed of the media.
    pub fn set_speed(&self, speed: f64) -> Result<(), Error> {
        self.write().set_speed(speed)
    }

    /// Get the current playback speed.
    pub fn speed(&self) -> f64 {
        self.read().speed
    }

    /// Get the current playback position in time.
    pub fn position(&self) -> Duration {
        Duration::from_nanos(
            self.read()
                .source
                .query_position::<gst::ClockTime>()
                .map_or(0, |pos| pos.nseconds()),
        )
    }

    /// Get the media duration.
    pub fn duration(&self) -> Duration {
        self.read().duration
    }

    /// Restarts a stream.
    pub fn restart_stream(&self) -> Result<(), Error> {
        self.write().restart_stream()
    }

    /// Get the underlying GStreamer pipeline.
    pub fn pipeline(&self) -> gst::Pipeline {
        self.read().source.clone()
    }

    /// Get the current NV12 frame data if available.
    pub fn current_frame_data(&self) -> Option<(Vec<u8>, u32, u32)> {
        let inner = self.read();

        // Check if we have frame data available
        if let Some(readable) = inner.frame.lock().readable() {
            let data = readable.as_slice().to_vec();
            if !data.is_empty() {
                return Some((data, inner.width as u32, inner.height as u32));
            }
        }

        None
    }

    /// Returns true if a new frame arrived since last check and resets the flag.
    pub fn take_frame_ready(&self) -> bool {
        self.read().upload_frame.swap(false, Ordering::SeqCst)
    }

    /// Configure the frame buffer capacity (0 disables buffering).
    pub fn set_frame_buffer_capacity(&self, capacity: usize) {
        let inner = self.read();
        inner
            .frame_buffer_capacity
            .store(capacity, Ordering::SeqCst);
        if capacity == 0 {
            inner.frame_buffer.lock().clear();
        } else {
            let mut buf = inner.frame_buffer.lock();
            while buf.len() > capacity {
                buf.pop_front();
            }
        }
    }

    /// Retrieve the current frame buffer capacity.
    pub fn frame_buffer_capacity(&self) -> usize {
        self.read().frame_buffer_capacity.load(Ordering::SeqCst)
    }

    /// Pop the oldest buffered frame, returning raw NV12 bytes with width/height.
    /// Returns None if the buffer is empty or mapping fails.
    pub fn pop_buffered_frame(&self) -> Option<(Vec<u8>, u32, u32)> {
        let (width, height) = self.size();
        let inner = self.read();
        let maybe_frame = inner.frame_buffer.lock().pop_front();
        if let Some(frame) = maybe_frame
            && let Some(readable) = frame.readable()
        {
            let data = readable.as_slice().to_vec();
            if !data.is_empty() {
                return Some((data, width as u32, height as u32));
            }
        }
        None
    }

    /// Number of frames currently buffered.
    pub fn buffered_len(&self) -> usize {
        self.read().frame_buffer.lock().len()
    }
}
