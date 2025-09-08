# `gpui-video-player`

A video player library for [gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui) applications, built on top of GStreamer. This library provides efficient video playback with hardware-accelerated rendering on supported platforms.

## Installation

### GStreamer Dependencies

This library requires GStreamer and its plugins to be installed on your system. Please refer to the [GStreamer Rust bindings installation guide](https://github.com/sdroege/gstreamer-rs?tab=readme-ov-file#installation) for detailed instructions.

**Minimum Requirements:**
- GStreamer 1.14+
- gst-plugins-base 1.14+
- gst-plugins-good (recommended)
- gst-plugins-bad (recommended for additional codec support)

### Adding to Your Project

Add this to your `Cargo.toml`:

```toml
[dependencies]
gpui-video-player = { git = "https://github.com/cijiugechu/gpui-video-player.git" }
```

**Note:** This library depends on GPUI, which must be available in your project. The current version requires GPUI from the Zed repository.

## Usage

### Basic Video Playback

```rust
use gpui::{App, Application, Context, Render, Window, WindowOptions, div, prelude::*};
use gpui_video_player::{Video, video};
use std::path::PathBuf;
use url::Url;

struct VideoPlayer {
    video: Video,
}

impl Render for VideoPlayer {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                video(self.video.clone())
                    .id("main-video")
                    .buffer_capacity(30), // Buffer up to 30 frames
            )
    }
}

fn main() {
    env_logger::init();
    Application::new().run(|cx: &mut App| {
        let uri = Url::from_file_path(
            PathBuf::from("./path/to/your/video.mp4"),
        ).expect("invalid file path");

        cx.open_window(
            WindowOptions {
                focus: true,
                ..Default::default()
            },
            |_, cx| {
                let video = Video::new(&uri).expect("failed to create video");
                cx.new(|_| VideoPlayer { video })
            },
        ).unwrap();
        cx.activate(true);
    });
}
```

### Video Controls

```rust
use gpui_video_player::Video;
use std::time::Duration;

// Playback control
video.set_paused(true);  // Pause
video.set_paused(false); // Resume

// Seeking
video.seek(Duration::from_secs(30), false)?; // Seek to 30 seconds

// Volume control
video.set_volume(0.5); // 50% volume
video.set_muted(true); // Mute

// Speed control
video.set_speed(2.0)?; // 2x speed

// Display size
video.set_display_width(Some(800)); // Override width
video.set_display_height(Some(600)); // Override height
```

### Advanced Configuration

```rust
use gpui_video_player::{Video, VideoOptions, video};

let options = VideoOptions {
    frame_buffer_capacity: Some(10), // Buffer 10 frames
    looping: Some(true),            // Enable looping
    speed: Some(1.5),               // 1.5x speed
};

let video = Video::new_with_options(&uri, options)?;
```

### Custom Video Element

```rust
use gpui_video_player::{VideoElement, video};

let video_element = video(my_video)
    .id("custom-video")
    .size(px(640.0), px(480.0))  // Set display size
    .buffer_capacity(5);         // Buffer 5 frames
```

## API Reference

### Video

The main video player struct with methods for:

- **Playback Control**: `set_paused()`, `paused()`
- **Seeking**: `seek()`, `position()`, `duration()`
- **Audio**: `set_volume()`, `volume()`, `set_muted()`, `muted()`
- **Speed**: `set_speed()`, `speed()`
- **Display**: `display_size()`, `set_display_size()`
- **Frame Access**: `current_frame_data()`, `take_frame_ready()`

### VideoElement

GPUI element for rendering video with:

- **Sizing**: `size()`, `width()`, `height()`
- **Buffering**: `buffer_capacity()`
- **Identification**: `id()`

### Position

Time or frame-based positioning:

```rust
use gpui_video_player::Position;
use std::time::Duration;

let time_pos = Position::Time(Duration::from_secs(10));
let frame_pos = Position::Frame(300);
```

## Examples

Run the included examples to see the library in action:

```bash
# Basic video player
cargo run --example video_player

# Video player with controls
cargo run --example with_controls
```

## Platform Notes

### macOS
- Uses `CVPixelBuffer` for hardware-accelerated rendering when possible
- Falls back to software rendering via GPUI sprite atlas

### Linux/Windows
- Uses optimized software rendering via GPUI sprite atlas
- Supports various GStreamer backends


## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
