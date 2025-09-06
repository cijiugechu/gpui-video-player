use crate::video::Video;
use gpui::{
    Context, IntoElement, ParentElement, Render, Styled, Window, div, prelude::StyledImage as _,
};
use yuvutils_rs::{
    YuvBiPlanarImage, YuvConversionMode, YuvRange, YuvStandardMatrix, yuv_nv12_to_rgba,
};

/// Advanced GPU-based video renderer that converts YUV to RGB on CPU as fallback
/// This provides a working solution while we develop full GPU integration
pub struct AdvancedGpuRenderer {
    video: Video,
    display_width: Option<gpui::Pixels>,
    display_height: Option<gpui::Pixels>,
}

impl AdvancedGpuRenderer {
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

    /// Convert NV12 YUV data to RGB using optimized yuvutils-rs
    fn yuv_to_rgb(&self, yuv_data: &[u8], width: u32, height: u32) -> Vec<u8> {
        let width_usize = width as usize;
        let height_usize = height as usize;
        let y_size = width_usize * height_usize;
        let uv_size = (width_usize * height_usize) / 2;

        if yuv_data.len() < y_size + uv_size {
            // Not enough data, return black frame
            return vec![0; width_usize * height_usize * 4];
        }

        // Split NV12 data into Y and UV planes
        let y_plane = &yuv_data[..y_size];
        let uv_plane = &yuv_data[y_size..y_size + uv_size];

        // Create YuvBiPlanarImage structure for NV12 data
        let yuv_bi_planar = YuvBiPlanarImage {
            y_plane,
            y_stride: width,
            uv_plane,
            uv_stride: width, // NV12 UV stride is same as width
            width,
            height,
        };

        // Prepare output RGB buffer (RGBA format)
        let mut rgba = vec![0u8; width_usize * height_usize * 4];
        let rgba_stride = width * 4;

        // Use yuvutils-rs optimized NV12 to RGB conversion
        // This uses SIMD optimizations (NEON, AVX2, AVX-512) when available
        // Try Bt709 first (HD standard) with full range
        if let Ok(_) = yuv_nv12_to_rgba(
            &yuv_bi_planar,
            &mut rgba,
            rgba_stride,
            YuvRange::Full,              // Try full range first
            YuvStandardMatrix::Bt709,    // HD standard
            YuvConversionMode::Balanced, // Use balanced conversion mode (default)
        ) {
            return rgba;
        }

        // Try Bt709 with limited range
        if let Ok(_) = yuv_nv12_to_rgba(
            &yuv_bi_planar,
            &mut rgba,
            rgba_stride,
            YuvRange::Limited,           // Limited range
            YuvStandardMatrix::Bt709,    // HD standard
            YuvConversionMode::Balanced, // Use balanced conversion mode (default)
        ) {
            return rgba;
        }

        // Fallback to Bt601 (SD standard)
        match yuv_nv12_to_rgba(
            &yuv_bi_planar,
            &mut rgba,
            rgba_stride,
            YuvRange::Limited,
            YuvStandardMatrix::Bt601,
            YuvConversionMode::Balanced, // Use balanced conversion mode (default)
        ) {
            Ok(_) => rgba,
            Err(_) => {
                // Final fallback to black frame on conversion error
                vec![0; width_usize * height_usize * 4]
            }
        }
    }
}

impl Render for AdvancedGpuRenderer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Always request continuous redraws to ensure smooth video playback
        // This is necessary for real-time video rendering
        cx.notify();

        let (display_width, display_height) = self.get_display_size();

        // Get the current frame data and convert to RGB
        if let Some((yuv_data, frame_width, frame_height)) = self.video.current_frame_data() {
            let rgb_data = self.yuv_to_rgb(&yuv_data, frame_width, frame_height);

            // Create GPUI image from RGB data
            use image::{ImageBuffer, Rgba};
            use smallvec::SmallVec;

            if let Some(image_buffer) =
                ImageBuffer::<Rgba<u8>, _>::from_raw(frame_width, frame_height, rgb_data)
            {
                let frames: SmallVec<[image::Frame; 1]> =
                    SmallVec::from_elem(image::Frame::new(image_buffer), 1);
                let render_image = std::sync::Arc::new(gpui::RenderImage::new(frames));

                div()
                    .w(display_width)
                    .h(display_height)
                    .child(gpui::img(render_image).object_fit(gpui::ObjectFit::Fill))
            } else {
                // Fallback to colored rectangle if image creation fails
                div()
                    .w(display_width)
                    .h(display_height)
                    .bg(gpui::blue())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(format!("YUV {}x{}", frame_width, frame_height))
            }
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

/// Helper function to create an advanced GPU video renderer
pub fn advanced_gpu_renderer(video: Video) -> AdvancedGpuRenderer {
    AdvancedGpuRenderer::new(video)
}
