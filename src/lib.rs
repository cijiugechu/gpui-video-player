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

mod error;
mod video;
mod video_player;
mod gpu_video_renderer;
mod advanced_gpu_renderer;

pub use error::Error;
pub use video::{Position, Video};
pub use video_player::{VideoPlayer, VideoPlayerEvent, VideoPlayerView, video_player, video_player_from_uri};
pub use gpu_video_renderer::{GpuVideoRenderer, gpu_video_renderer};
pub use advanced_gpu_renderer::{AdvancedGpuRenderer, advanced_gpu_renderer};

// Re-export commonly used types
pub use url::Url;
pub use gstreamer as gst;