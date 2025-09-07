use crate::video::Video;
use gpui::{
    Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Window,
};
use yuv::{YuvBiPlanarImage, YuvConversionMode, YuvRange, YuvStandardMatrix, yuv_nv12_to_bgra};

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

    /// Set only width; height is inferred via aspect ratio.
    pub fn width(mut self, width: gpui::Pixels) -> Self {
        self.display_width = Some(width);
        self.display_height = None;
        self
    }

    /// Set only height; width is inferred via aspect ratio.
    pub fn height(mut self, height: gpui::Pixels) -> Self {
        self.display_height = Some(height);
        self.display_width = None;
        self
    }

    /// Configure how many frames to buffer inside the underlying `Video`.
    /// 0 disables buffering and behaves like immediate rendering.
    pub fn buffer_capacity(self, capacity: usize) -> Self {
        self.video.set_frame_buffer_capacity(capacity);
        self
    }

    /// Get the current display dimensions, falling back to video's effective display size.
    fn get_display_size(&self) -> (gpui::Pixels, gpui::Pixels) {
        match (self.display_width, self.display_height) {
            (Some(w), Some(h)) => (w, h),
            _ => {
                let (w, h) = self.video.display_size();
                (gpui::px(w as f32), gpui::px(h as f32))
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

        // Prepare output RGB buffer (BGRA format)
        let mut bgra = vec![0u8; width_usize * height_usize * 4];
        let rgba_stride = width * 4;

        // Use yuvutils-rs optimized NV12 to RGB conversion
        // Try Bt709 first (HD standard) with full range
        if let Ok(_) = yuv_nv12_to_bgra(
            &yuv_bi_planar,
            &mut bgra,
            rgba_stride,
            YuvRange::Full,              // Try full range first
            YuvStandardMatrix::Bt709,    // HD standard
            YuvConversionMode::Balanced, // Use balanced conversion mode (default)
        ) {
            return bgra;
        }

        // Try Bt709 with limited range
        if let Ok(_) = yuv_nv12_to_bgra(
            &yuv_bi_planar,
            &mut bgra,
            rgba_stride,
            YuvRange::Limited,           // Limited range
            YuvStandardMatrix::Bt709,    // HD standard
            YuvConversionMode::Balanced, // Use balanced conversion mode (default)
        ) {
            return bgra;
        }

        // Fallback to Bt601 (SD standard)
        match yuv_nv12_to_bgra(
            &yuv_bi_planar,
            &mut bgra,
            rgba_stride,
            YuvRange::Limited,
            YuvStandardMatrix::Bt601,
            YuvConversionMode::Balanced, // Use balanced conversion mode (default)
        ) {
            Ok(_) => bgra,
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
        let (mut width, mut height) = self.get_display_size();

        // Also honor any video-level display overrides if element-level not specified
        if self.display_width.is_none() || self.display_height.is_none() {
            let (vw, vh) = self.video.display_size();
            if self.display_width.is_none() {
                width = gpui::px(vw as f32);
            }
            if self.display_height.is_none() {
                height = gpui::px(vh as f32);
            }
        }

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
        // Prefer buffered frames if available. Drain to the latest to avoid lag.
        let buffered = self.video.buffered_len();
        let mut frame_to_render: Option<(Vec<u8>, u32, u32)> = None;
        let mut from_buffer = false;
        if buffered > 0 {
            for _ in 0..buffered {
                if let Some(frame) = self.video.pop_buffered_frame() {
                    frame_to_render = Some(frame);
                }
            }
            from_buffer = frame_to_render.is_some();
        } else {
            frame_to_render = self.video.current_frame_data();
        }

        if let Some((yuv_data, frame_width, frame_height)) = frame_to_render {
            if from_buffer {
                log::debug!(
                    "Painting frame from buffer (buffered_len before drain: {})",
                    buffered
                );
            } else {
                log::debug!("Painting frame from live current_frame_data()");
            }
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

                // Compute aspect-fit bounds inside the provided bounds to avoid stretching
                let container_w = bounds.size.width.0;
                let container_h = bounds.size.height.0;
                let frame_w = frame_width as f32;
                let frame_h = frame_height as f32;

                let scale = if frame_w > 0.0 && frame_h > 0.0 {
                    (container_w / frame_w).min(container_h / frame_h)
                } else {
                    1.0
                };

                let dest_w = (frame_w * scale).max(0.0);
                let dest_h = (frame_h * scale).max(0.0);
                let offset_x = (container_w - dest_w) * 0.5;
                let offset_y = (container_h - dest_h) * 0.5;

                let dest_bounds = gpui::Bounds::new(
                    gpui::point(
                        bounds.origin.x + gpui::px(offset_x),
                        bounds.origin.y + gpui::px(offset_y),
                    ),
                    gpui::size(gpui::px(dest_w), gpui::px(dest_h)),
                );

                // Paint the image within the fitted bounds (letterboxed/pillarboxed)
                window
                    .paint_image(
                        dest_bounds,
                        gpui::Corners::default(),
                        render_image,
                        0,
                        false,
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
