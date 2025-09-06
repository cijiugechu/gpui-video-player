use crate::video::Video;
use gpui::{
    div, px, Context, IntoElement, ParentElement, Render, Styled, Window,
    prelude::StyledImage as _,
};
use std::sync::atomic::Ordering;

/// Advanced GPU-based video renderer that converts YUV to RGB on CPU as fallback
/// This provides a working solution while we develop full GPU integration
pub struct AdvancedGpuRenderer {
    video: Video,
}

impl AdvancedGpuRenderer {
    pub fn new(video: Video) -> Self {
        Self { video }
    }

    pub fn video(&self) -> &Video {
        &self.video
    }

    /// Convert NV12 YUV data to RGB on CPU
    fn yuv_to_rgb(&self, yuv_data: &[u8], width: u32, height: u32) -> Vec<u8> {
        let width = width as usize;
        let height = height as usize;
        let y_size = width * height;
        
        if yuv_data.len() < y_size + (width * height / 2) {
            // Not enough data, return black frame
            return vec![0; width * height * 4];
        }

        let mut rgb_data = vec![0u8; width * height * 4];
        
        for y in 0..height {
            for x in 0..width {
                let y_index = y * width + x;
                let uv_index = y_size + (y / 2) * width + (x & !1);
                
                if y_index >= y_size || uv_index + 1 >= yuv_data.len() {
                    continue;
                }
                
                let y_val = yuv_data[y_index] as f32;
                let u_val = yuv_data[uv_index] as f32;
                let v_val = yuv_data[uv_index + 1] as f32;
                
                // YUV to RGB conversion (ITU-R BT.601)
                let c = y_val - 16.0;
                let d = u_val - 128.0;
                let e = v_val - 128.0;
                
                let r = (1.164 * c + 1.596 * e).clamp(0.0, 255.0) as u8;
                let g = (1.164 * c - 0.392 * d - 0.813 * e).clamp(0.0, 255.0) as u8;
                let b = (1.164 * c + 2.017 * d).clamp(0.0, 255.0) as u8;
                
                let rgb_index = (y * width + x) * 4;
                rgb_data[rgb_index] = r;
                rgb_data[rgb_index + 1] = g;
                rgb_data[rgb_index + 2] = b;
                rgb_data[rgb_index + 3] = 255; // Alpha
            }
        }
        
        rgb_data
    }
}

impl Render for AdvancedGpuRenderer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Check if we have a new frame and request redraw if so
        let inner = self.video.read();
        if inner.upload_frame.swap(false, Ordering::SeqCst) {
            cx.notify();
        }

        let (width, height) = self.video.size();
        
        // Get the current frame data and convert to RGB
        if let Some((yuv_data, frame_width, frame_height)) = self.video.current_frame_data() {
            let rgb_data = self.yuv_to_rgb(&yuv_data, frame_width, frame_height);
            
            // Create GPUI image from RGB data
            use image::{ImageBuffer, Rgba};
            use smallvec::SmallVec;
            
            if let Some(image_buffer) = ImageBuffer::<Rgba<u8>, _>::from_raw(
                frame_width, 
                frame_height, 
                rgb_data
            ) {
                let frames: SmallVec<[image::Frame; 1]> = SmallVec::from_elem(
                    image::Frame::new(image_buffer), 
                    1
                );
                let render_image = std::sync::Arc::new(gpui::RenderImage::new(frames));
                
                div()
                    .w(px(width as f32))
                    .h(px(height as f32))
                    .child(gpui::img(render_image).object_fit(gpui::ObjectFit::Contain))
            } else {
                // Fallback to colored rectangle if image creation fails
                div()
                    .w(px(width as f32))
                    .h(px(height as f32))
                    .bg(gpui::blue())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(format!("YUV {}x{}", frame_width, frame_height))
            }
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

/// Helper function to create an advanced GPU video renderer
pub fn advanced_gpu_renderer(video: Video) -> AdvancedGpuRenderer {
    AdvancedGpuRenderer::new(video)
}
