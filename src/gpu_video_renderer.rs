use crate::video::Video;
use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};
use std::sync::atomic::Ordering;

/// Simple GPU-based video renderer placeholder
/// This is a simplified version that works with GPUI's current Element system
pub struct GpuVideoRenderer {
    video: Video,
    display_width: Option<gpui::Pixels>,
    display_height: Option<gpui::Pixels>,
}

impl GpuVideoRenderer {
    pub fn new(video: Video) -> Self {
        Self {
            video,
            display_width: None,
            display_height: None,
        }
    }

    pub fn video(&self) -> &Video {
        &self.video
    }

    /// Set the display dimensions for the video renderer.
    pub fn set_display_size(&mut self, width: gpui::Pixels, height: gpui::Pixels) {
        self.display_width = Some(width);
        self.display_height = Some(height);
    }

    /// Get the current display dimensions, falling back to video natural size.
    fn get_display_size(&self) -> (gpui::Pixels, gpui::Pixels) {
        match (self.display_width, self.display_height) {
            (Some(w), Some(h)) => (w, h),
            _ => {
                let (video_width, video_height) = self.video.size();
                (gpui::px(video_width as f32), gpui::px(video_height as f32))
            }
        }
    }
}

impl Render for GpuVideoRenderer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Check if we have a new frame and request redraw if so
        let inner = self.video.read();
        if inner.upload_frame.swap(false, Ordering::SeqCst) {
            cx.notify();
        }

        let (display_width, display_height) = self.get_display_size();

        // Get the current frame data
        if let Some((_yuv_data, _frame_width, _frame_height)) = self.video.current_frame_data() {
            // For now, show a green rectangle to indicate video is playing
            // TODO: Implement actual GPU YUV rendering with WGSL shaders
            div()
                .w(display_width)
                .h(display_height)
                .bg(gpui::green())
                .flex()
                .items_center()
                .justify_center()
                .child("▶ Video Playing")
        } else {
            // No frame available - show loading state
            div()
                .w(display_width)
                .h(display_height)
                .bg(gpui::black())
                .flex()
                .items_center()
                .justify_center()
                .child("Loading video...")
        }
    }
}

/// Helper function to create a GPU video renderer
pub fn gpu_video_renderer(video: Video) -> GpuVideoRenderer {
    GpuVideoRenderer::new(video)
}
