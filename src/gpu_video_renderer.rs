use crate::video::Video;
use gpui::{
    div, px, Context, IntoElement, ParentElement, Render, Styled, Window,
};
use std::sync::atomic::Ordering;

/// Simple GPU-based video renderer placeholder
/// This is a simplified version that works with GPUI's current Element system
pub struct GpuVideoRenderer {
    video: Video,
}

impl GpuVideoRenderer {
    pub fn new(video: Video) -> Self {
        Self { video }
    }

    pub fn video(&self) -> &Video {
        &self.video
    }
}

impl Render for GpuVideoRenderer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Check if we have a new frame and request redraw if so
        let inner = self.video.read();
        if inner.upload_frame.swap(false, Ordering::SeqCst) {
            cx.notify();
        }

        let (width, height) = self.video.size();
        
        // Get the current frame data
        if let Some((_yuv_data, _frame_width, _frame_height)) = self.video.current_frame_data() {
            // For now, show a green rectangle to indicate video is playing
            // TODO: Implement actual GPU YUV rendering with WGSL shaders
            div()
                .w(px(width as f32))
                .h(px(height as f32))
                .bg(gpui::green())
                .flex()
                .items_center()
                .justify_center()
                .child("▶ Video Playing")
        } else {
            // No frame available - show loading state
            div()
                .w(px(width as f32))
                .h(px(height as f32))
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
