use crate::video::Video;
use gpui::{
    Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Window,
};
use yuv::{YuvBiPlanarImage, YuvConversionMode, YuvRange, YuvStandardMatrix, yuv_nv12_to_rgba};

/// A video element that implements Element trait similar to GPUI's img element
pub struct VideoElement {
    video: Video,
    display_width: Option<gpui::Pixels>,
    display_height: Option<gpui::Pixels>,
    element_id: Option<ElementId>,
}

impl VideoElement {
    pub fn new(video: Video) -> Self {
        Self {
            video,
            display_width: None,
            display_height: None,
            element_id: None,
        }
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.element_id = Some(id.into());
        self
    }

    pub fn size(mut self, width: gpui::Pixels, height: gpui::Pixels) -> Self {
        self.display_width = Some(width);
        self.display_height = Some(height);
        self
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

impl Element for VideoElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        self.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let (width, height) = self.get_display_size();

        let style = gpui::Style {
            size: gpui::Size {
                width: gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                    gpui::AbsoluteLength::Pixels(width),
                )),
                height: gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                    gpui::AbsoluteLength::Pixels(height),
                )),
            },
            ..Default::default()
        };

        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout_state: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut gpui::App,
    ) -> Self::PrepaintState {
        // Schedule repaints only when playing or when a new frame arrived.
        let is_playing = !self.video.eos() && !self.video.paused();
        let has_new_frame = self.video.take_frame_ready();
        if is_playing || has_new_frame {
            window.request_animation_frame();
        }
        ()
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout_state: &mut Self::RequestLayoutState,
        _prepaint_state: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut gpui::App,
    ) {
        // Get the current frame data and render it
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

                // Paint the image to fill the bounds
                window
                    .paint_image(
                        bounds,
                        gpui::Corners::default(),
                        render_image,
                        0,     // frame index
                        false, // grayscale
                    )
                    .ok();
            }
        }
    }
}

impl IntoElement for VideoElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Helper function to create a video element
pub fn video(video: Video) -> VideoElement {
    VideoElement::new(video)
}
