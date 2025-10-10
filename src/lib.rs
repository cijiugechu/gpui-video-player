//! A video player library for [gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui)
//! applications, built on top of GStreamer.
//!
//! This library provides efficient video playback with hardware-accelerated rendering
//! on supported platforms (macOS uses CVPixelBuffer when available).
//!
//! # Prerequisites
//!
//! GStreamer 1.14+ and its plugins must be installed on your system.
//! See the [GStreamer Rust bindings installation guide](https://github.com/sdroege/gstreamer-rs?tab=readme-ov-file#installation)
//! for platform-specific instructions.
//!
//! # Example
//!
//! ```no_run
//! use gpui::{App, Application, Context, Render, Window, WindowOptions, div, prelude::*};
//! use gpui_video_player::{Video, video};
//! use std::path::PathBuf;
//! use url::Url;
//!
//! struct VideoPlayer {
//!     video: Video,
//! }
//!
//! impl Render for VideoPlayer {
//!     fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
//!         div()
//!             .size_full()
//!             .flex()
//!             .items_center()
//!             .justify_center()
//!             .child(
//!                 video(self.video.clone())
//!                     .id("main-video")
//!                     .buffer_capacity(30)
//!             )
//!     }
//! }
//!
//! fn main() {
//!     Application::new().run(|cx: &mut App| {
//!         let uri = Url::from_file_path(
//!             PathBuf::from("./video.mp4")
//!         ).expect("invalid file path");
//!
//!         cx.open_window(WindowOptions::default(), |_, cx| {
//!             let video = Video::new(&uri).expect("failed to create video");
//!             cx.new(|_| VideoPlayer { video })
//!         }).unwrap();
//!     });
//! }
//! ```
//!
//! # Playback Control
//!
//! The [`Video`] handle provides methods for controlling playback:
//!
//! ```no_run
//! # use gpui_video_player::Video;
//! # use url::Url;
//! # let uri = Url::parse("file:///video.mp4").unwrap();
//! # let video = Video::new(&uri).unwrap();
//! use std::time::Duration;
//!
//! video.set_paused(true);              // Pause playback
//! video.seek(Duration::from_secs(30), false).ok(); // Seek to 30s
//! video.set_volume(0.5);               // Set volume to 50%
//! video.set_speed(1.5).ok();           // Play at 1.5x speed
//! ```
//!
//! See the `examples/` directory for more complete usage patterns.

mod element;
mod error;
mod video;

pub use element::{VideoElement, video};
pub use error::Error;
pub use video::{Position, Video, VideoOptions};

// Re-export commonly used types
pub use gstreamer as gst;
pub use url::Url;
