use crate::video::Video;
use gpui::{
    AppContext, Context, Entity, EventEmitter, IntoElement, ParentElement, Render, Styled, Window,
    div,
};
use gstreamer as gst;
use std::sync::atomic::Ordering;

/// Content fit modes for video display.
#[derive(Debug, Clone, Copy)]
pub enum ContentFit {
    Contain,
    Cover,
    Fill,
    ScaleDown,
    None,
}

/// Events that can be emitted by the video player.
#[derive(Debug, Clone)]
pub enum VideoPlayerEvent {
    /// Video reached end of stream.
    EndOfStream,
    /// New frame is available.
    NewFrame,
    /// Error occurred during playback.
    Error(String),
    /// Subtitle text changed.
    SubtitleText(Option<String>),
}

/// Video player component for GPUI.
#[derive(Debug)]
pub struct VideoPlayer {
    video: Video,
    width: Option<gpui::Pixels>,
    height: Option<gpui::Pixels>,
    fit: ContentFit,
}

impl VideoPlayer {
    /// Create a new video player from a video URI.
    pub fn new(uri: &url::Url) -> Result<Self, crate::Error> {
        let video = Video::new(uri)?;
        Ok(Self {
            video,
            width: None,
            height: None,
            fit: ContentFit::Contain,
        })
    }

    /// Create a video player from an existing Video instance.
    pub fn from_video(video: Video) -> Self {
        Self {
            video,
            width: None,
            height: None,
            fit: ContentFit::Contain,
        }
    }

    /// Set the width of the video player.
    pub fn width(mut self, width: gpui::Pixels) -> Self {
        self.width = Some(width);
        self
    }

    /// Set the height of the video player.
    pub fn height(mut self, height: gpui::Pixels) -> Self {
        self.height = Some(height);
        self
    }

    /// Set the content fit mode.
    pub fn content_fit(mut self, fit: ContentFit) -> Self {
        self.fit = fit;
        self
    }

    /// Get a reference to the underlying video.
    pub fn video(&self) -> &Video {
        &self.video
    }

    /// Get a mutable reference to the underlying video.
    pub fn video_mut(&mut self) -> &Video {
        &self.video
    }

    /// Check if a new frame is available.
    fn has_new_frame(&self) -> bool {
        let inner = self.video.read();
        inner.upload_frame.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Check for GStreamer bus messages and handle events.
    fn handle_bus_messages(&self, cx: &mut Context<VideoPlayerView>) {
        let inner = self.video.read();

        while let Some(msg) = inner
            .bus
            .pop_filtered(&[gst::MessageType::Error, gst::MessageType::Eos])
        {
            match msg.view() {
                gst::MessageView::Error(err) => {
                    log::error!("GStreamer error: {}", err.error());
                    cx.emit(VideoPlayerEvent::Error(err.error().to_string()));
                }
                gst::MessageView::Eos(_) => {
                    cx.emit(VideoPlayerEvent::EndOfStream);
                }
                _ => {}
            }
        }

        // Check for new frames once (consume the flag) and schedule redraw
        if inner.upload_frame.swap(false, Ordering::SeqCst) {
            cx.emit(VideoPlayerEvent::NewFrame);
            cx.notify();
        }

        // Check for subtitle updates
        if inner.upload_text.swap(false, Ordering::SeqCst) {
            let text = inner.subtitle_text.lock().clone();
            cx.emit(VideoPlayerEvent::SubtitleText(text));
        }
    }
}

/// A view wrapper for the VideoPlayer component.
pub struct VideoPlayerView {
    player: VideoPlayer,
    gpu_renderer: Option<Entity<crate::gpu_video_renderer::GpuVideoRenderer>>,
}

impl VideoPlayerView {
    /// Create a new video player view.
    pub fn new(player: VideoPlayer) -> Self {
        Self {
            player,
            gpu_renderer: None,
        }
    }

    /// Get a reference to the video player.
    pub fn player(&self) -> &VideoPlayer {
        &self.player
    }

    /// Get a mutable reference to the video player.
    pub fn player_mut(&mut self) -> &VideoPlayer {
        &self.player
    }
}

impl EventEmitter<VideoPlayerEvent> for VideoPlayerView {}

impl Render for VideoPlayerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::gpu_video_renderer::gpu_video_renderer;

        // Handle GStreamer events
        self.player.handle_bus_messages(cx);

        // Only request redraw when we have a new frame
        if self.player.has_new_frame() {
            cx.notify();
        }

        // Create or reuse the GPU renderer entity
        if self.gpu_renderer.is_none() {
            let renderer = gpu_video_renderer(self.player.video.clone());
            self.gpu_renderer = Some(cx.new(|_| renderer));
        }

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(self.gpu_renderer.as_ref().unwrap().clone())
    }
}

/// Helper function to create a video player view.
pub fn video_player(player: VideoPlayer) -> VideoPlayerView {
    VideoPlayerView::new(player)
}

/// Helper function to create a video player from a URI.
pub fn video_player_from_uri(uri: &url::Url) -> Result<VideoPlayerView, crate::Error> {
    let player = VideoPlayer::new(uri)?;
    Ok(video_player(player))
}
