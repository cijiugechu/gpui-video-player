//! # GPUI Video Player
//!
//! A video player component for GPUI using GStreamer for media playback.
//!
//! ## Features
//!
//! - GStreamer-powered video decoding and playback
//! - CPU-based YUV to RGBA conversion (no GPU shaders required)
//! - Support for various video formats and codecs
//! - Subtitle support
//! - Audio control (volume, muting)
//! - Playback control (play, pause, seek, speed)
//! - Event-driven architecture for handling video events
//!
//! ## Example
//!
//! ```rust
//! use gpui_video_player::{VideoPlayer, VideoPlayerEvent, video_player_from_uri};
//! use gpui::{App, AppContext, Entity, EventEmitter};
//! use url::Url;
//!
//! fn main() {
//!     let uri = Url::parse("file:///path/to/video.mp4").unwrap();
//!     let player = video_player_from_uri(&uri).unwrap();
//!     
//!     // Use the player in your GPUI application
//! }
//! ```

mod advanced_gpu_renderer;
mod error;
mod video;
mod video_player;

pub use advanced_gpu_renderer::{
    AdvancedGpuRenderer, VideoElement, advanced_gpu_renderer, video_element,
};
pub use error::Error;
pub use video::{Position, Video};
pub use video_player::{
    ContentFit, VideoPlayer, VideoPlayerEvent, VideoPlayerView, video_player, video_player_from_uri,
};

// Re-export commonly used types
pub use gstreamer as gst;
pub use url::Url;
