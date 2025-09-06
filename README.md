## GPUI Video Player (GStreamer, CPU Rendering)

### Overview

This crate provides a custom video player component for GPUI. It uses GStreamer for media decoding and a CPU path to convert decoded NV12 frames to RGBA, which are then displayed as GPUI images. No WGPU/WGSL shaders are used.

### High-level Architecture

- **GStreamer pipeline**: A `playbin` is constructed with a video `appsink` that outputs NV12 (`video/x-raw,format=NV12`) and an optional text `appsink` for subtitles.
- **Worker thread**: A dedicated thread pulls decoded frames from the `appsink`, converts the NV12 frame to RGBA on the CPU, and stores the result in shared state.
- **GPUI view**: `VideoPlayerView` reads the latest RGBA buffer, constructs a `gpui::RenderImage`, and renders it via `gpui::img(...)` each frame.
- **Events**: Errors, EOS, new frame, and subtitle updates are emitted as GPUI events.

---

## Components

### `src/video.rs` — GStreamer integration and media control

- **Pipeline construction**
  - Initializes GStreamer, builds a `playbin` pipeline with:
    - `video-sink = "videoscale ! videoconvert ! appsink name=gpui_video drop=true caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1"`
    - `text-sink  = "appsink name=gpui_text sync=true drop=true"` (optional)
  - Extracts width/height/framerate from negotiated caps.

- **Worker thread (frame pull + CPU conversion)**
  - Chooses `try_pull_preroll` while not playing, otherwise `try_pull_sample` each ~16ms.
  - For each sample:
    1. Map the GStreamer buffer as readable bytes.
    2. Convert NV12 → RGBA using `yuv_to_rgba` on CPU.
    3. Store the RGBA `Vec<u8>` in `rgba_frame` along with flags to signal a new frame.
    4. Pop subtitle samples when available and synchronize them to video segment times.

- **CPU color conversion (NV12 → RGBA)**
  - NV12 layout: full-res Y plane, followed by interleaved UV plane at half resolution.
  - Per-pixel conversion (BT.709-style coefficients):
    - r = 1.164·(Y−16) + 1.596·(V−128)
    - g = 1.164·(Y−16) − 0.813·(V−128) − 0.391·(U−128)
    - b = 1.164·(Y−16) + 2.018·(U−128)

- **Controls and queries**
  - `set_paused`, `seek(position, accurate)`, `set_speed`, `set_volume`, `set_muted`, `set_looping`.
  - `position()`, `duration()`, `framerate()`, `size()`.
  - `pipeline()` for access to the underlying `gst::Pipeline` when needed.

### `src/video_player.rs` — GPUI component and rendering

- **Events**: `VideoPlayerEvent::{EndOfStream, NewFrame, Error(String), SubtitleText(Option<String>)}`.
- **Bus handling**: During `render()`, `VideoPlayerView` drains `Error`/`Eos` messages from the GStreamer bus and emits events.
- **Redraw**: Calls `cx.notify()` each render so the view keeps updating.

#### Rendering pipeline to final image

1. Read the latest RGBA buffer produced by the worker thread: `Video::current_frame() -> Option<(Vec<u8>, u32, u32)>`.
2. Convert RGBA → BGRA in-place (swap R/B) to match GPUI’s expected pixel order for `RenderImage`.
3. Build an `image::ImageBuffer<Rgba<u8>>` from the raw data and wrap it in a single `image::Frame`.
4. Create a `smallvec::SmallVec<[Frame; 1]>` with the single frame and construct a `gpui::RenderImage` from it.
5. Render the image using `gpui::img(render_image).object_fit(gpui::ObjectFit::Contain)` inside a layouting `div()`.

This results in the decoded video frame being presented as a standard GPUI image element, scaled and fitted according to the selected content fit.

### Example

See `examples/minimal.rs` for a runnable app that opens a window and displays a video:

```rust
let uri = Url::from_file_path(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("./assets/test.mp4"))
    .expect("invalid file path");

cx.open_window(
    WindowOptions { focus: true, ..Default::default() },
    |_, cx| {
        let view = video_player_from_uri(&uri).expect("failed to create player");
        let view_entity = cx.new(|_| view);
        cx.new(|_| PlayerExample::new(view_entity))
    },
)?;
```

---

## Error handling

- Uses `thiserror` for domain errors.
- Avoids panics; errors are propagated with `Result` where relevant.
- Worker thread logs unexpected failures and continues pulling frames while the `alive` flag remains set.

## Performance and limitations

- CPU-based conversion is simple and portable but is more expensive than a GPU shader path. For high resolutions/framerates, expect measurable CPU usage.
- Frames are copied into a new `RenderImage` each render; further optimizations (pooling or partial updates) are possible.
- Hardware acceleration (via GPU shaders or GL/Vulkan sinks) is not used in this implementation by design.

## File map

- `src/video.rs` — GStreamer pipeline, worker thread, CPU NV12→RGBA conversion, media controls.
- `src/video_player.rs` — GPUI component (`VideoPlayerView`), event handling, RGBA→BGRA, `RenderImage` creation, and `img(...)` display.
- `examples/minimal.rs` — Minimal app that opens a GPUI window and displays a video.
- `Cargo.toml` — Dependencies: `gpui`, `gstreamer`, `gstreamer-app`, `gstreamer-base`, `glib`, `parking_lot`, `image`, `smallvec`, `url`, `thiserror` (+ `env_logger` for examples).

---

## Future work

- Zero-copy or reduced-copy uploads to `RenderImage`.
- Optional GPU conversion path for better performance at high resolutions.
- Subtitle overlay composited in the final image or as a separate text layer.

