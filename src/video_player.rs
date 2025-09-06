use crate::video::Video;
use gpui::{
    AppContext, Context, Entity, EventEmitter, IntoElement, ParentElement, Render, Styled, Window,
    div,
};
use gstreamer as gst;
use std::sync::atomic::Ordering;

/// Content fit modes for video display.
#[derive(Debug, Clone, Copy, PartialEq)]
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

    /// Get the specified width of the video player.
    pub fn get_width(&self) -> Option<gpui::Pixels> {
        self.width
    }

    /// Get the specified height of the video player.
    pub fn get_height(&self) -> Option<gpui::Pixels> {
        self.height
    }

    /// Get the content fit mode.
    pub fn get_content_fit(&self) -> ContentFit {
        self.fit
    }

    /// Calculate the display dimensions based on video size, specified dimensions, and content fit.
    pub fn calculate_display_size(&self) -> (gpui::Pixels, gpui::Pixels) {
        let (video_width, video_height) = self.video.size();
        let video_aspect = video_width as f32 / video_height as f32;

        match (self.get_width(), self.get_height()) {
            (Some(w), Some(h)) => {
                // Both width and height specified - apply ContentFit
                let container_aspect = w.0 / h.0;

                match self.fit {
                    ContentFit::Fill => (w, h),
                    ContentFit::Contain => {
                        if video_aspect > container_aspect {
                            // Video is wider - fit to width
                            (w, gpui::px(w.0 / video_aspect))
                        } else {
                            // Video is taller - fit to height
                            (gpui::px(h.0 * video_aspect), h)
                        }
                    }
                    ContentFit::Cover => {
                        if video_aspect > container_aspect {
                            // Video is wider - fit to height
                            (gpui::px(h.0 * video_aspect), h)
                        } else {
                            // Video is taller - fit to width
                            (w, gpui::px(w.0 / video_aspect))
                        }
                    }
                    ContentFit::ScaleDown => {
                        let natural_width = gpui::px(video_width as f32);
                        let natural_height = gpui::px(video_height as f32);

                        if natural_width.0 <= w.0 && natural_height.0 <= h.0 {
                            // Video fits naturally
                            (natural_width, natural_height)
                        } else {
                            // Scale down using contain logic
                            if video_aspect > container_aspect {
                                (w, gpui::px(w.0 / video_aspect))
                            } else {
                                (gpui::px(h.0 * video_aspect), h)
                            }
                        }
                    }
                    ContentFit::None => {
                        (gpui::px(video_width as f32), gpui::px(video_height as f32))
                    }
                }
            }
            (Some(w), None) => {
                // Only width specified - maintain aspect ratio
                (w, gpui::px(w.0 / video_aspect))
            }
            (None, Some(h)) => {
                // Only height specified - maintain aspect ratio
                (gpui::px(h.0 * video_aspect), h)
            }
            (None, None) => {
                // No dimensions specified - use natural size
                (gpui::px(video_width as f32), gpui::px(video_height as f32))
            }
        }
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
    gpu_renderer: Option<Entity<crate::advanced_gpu_renderer::AdvancedGpuRenderer>>,
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
        use crate::advanced_gpu_renderer::advanced_gpu_renderer;

        // Handle GStreamer events
        self.player.handle_bus_messages(cx);

        // Always ensure we're rendering - the renderer will handle frame updates
        cx.notify();

        // Create or reuse the GPU renderer entity
        if self.gpu_renderer.is_none() {
            let renderer = advanced_gpu_renderer(self.player.video.clone());
            self.gpu_renderer = Some(cx.new(|_| renderer));
        }

        let (display_width, display_height) = self.player.calculate_display_size();
        let content_fit = self.player.get_content_fit();

        // Create the container based on specified dimensions
        let container = match (self.player.get_width(), self.player.get_height()) {
            (Some(w), Some(h)) => {
                // Both dimensions specified - create container with exact size
                let mut container_div = div()
                    .w(gpui::px(w.0))
                    .h(gpui::px(h.0))
                    .flex()
                    .items_center()
                    .justify_center();

                if content_fit == ContentFit::Cover {
                    // For Cover mode, clip overflow
                    container_div = container_div.overflow_hidden();
                }
                container_div
            }
            (Some(w), None) => {
                // Only width specified
                div()
                    .w(gpui::px(w.0))
                    .flex()
                    .items_center()
                    .justify_center()
            }
            (None, Some(h)) => {
                // Only height specified
                div()
                    .h(gpui::px(h.0))
                    .flex()
                    .items_center()
                    .justify_center()
            }
            (None, None) => {
                // No dimensions specified - size to content
                div().flex().items_center().justify_center()
            }
        };

        // Update the GPU renderer with the calculated dimensions
        if let Some(renderer) = &self.gpu_renderer {
            renderer.update(cx, |renderer, _cx| {
                renderer.set_display_size(display_width, display_height);
            });
        }

        container.child(self.gpu_renderer.as_ref().unwrap().clone())
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
