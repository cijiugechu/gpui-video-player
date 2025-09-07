use gpui::{App, Application, Context, Render, Window, WindowOptions, div, prelude::*};
use gpui_video_player::{Video, video};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use url::Url;

struct WithControlsExample {
    video: Video,
    last_click: Option<Instant>,
}

impl WithControlsExample {
    fn new(video: Video) -> Self {
        Self {
            video,
            last_click: None,
        }
    }

    fn click_allowed(&mut self) -> bool {
        let now = Instant::now();
        if let Some(prev) = self.last_click {
            if now.saturating_duration_since(prev) < Duration::from_millis(250) {
                return false;
            }
        }
        self.last_click = Some(now);
        true
    }
}

impl Render for WithControlsExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_paused = self.video.paused();
        let play_label = if is_paused {
            "▶️ Play"
        } else {
            "⏸️ Pause"
        };

        let back_5s = div()
            .id("btn-back-5s")
            .px_6()
            .py_3()
            .border_1()
            .cursor_pointer()
            .child("⏪ 5s")
            .on_click(cx.listener(|this: &mut Self, _event, _window, cx| {
                if !this.click_allowed() {
                    return;
                }
                let pos = this.video.position();
                let new_pos = pos.saturating_sub(Duration::from_secs(5));
                let _ = this.video.seek(new_pos, false);
                this.video.clear_frame_buffer();
                cx.notify();
            }));

        let play_pause = div()
            .id("btn-play-pause")
            .px_8()
            .py_4()
            .border_1()
            .cursor_pointer()
            .child(play_label)
            .on_click(cx.listener(|this: &mut Self, _event, _window, cx| {
                if !this.click_allowed() {
                    return;
                }
                let paused = this.video.paused();
                this.video.set_paused(!paused);
                cx.notify();
            }));

        let forward_5s = div()
            .id("btn-forward-5s")
            .px_6()
            .py_3()
            .border_1()
            .cursor_pointer()
            .child("5s ⏩")
            .on_click(cx.listener(|this: &mut Self, _event, _window, cx| {
                if !this.click_allowed() {
                    return;
                }
                let pos = this.video.position();
                let dur = this.video.duration();
                let target = pos.saturating_add(Duration::from_secs(5));
                let new_pos = if target > dur { dur } else { target };
                let _ = this.video.seek(new_pos, false);
                this.video.clear_frame_buffer();
                cx.notify();
            }));

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .relative()
                    .child(
                        video(self.video.clone())
                            .id("controlled-video")
                            .buffer_capacity(3),
                    )
                    .child(
                        div()
                            .absolute()
                            .size_full()
                            .flex()
                            .items_start()
                            .justify_center()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(back_5s)
                                    .child(play_pause)
                                    .child(forward_5s),
                            ),
                    ),
            )
    }
}

fn main() {
    env_logger::init();
    Application::new().run(|cx: &mut App| {
        let uri = Url::from_file_path(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("./assets/test3.mp4"),
        )
        .expect("invalid file path");

        let _ = cx.open_window(
            WindowOptions {
                focus: true,
                ..Default::default()
            },
            |_, cx| {
                let video = Video::new(&uri).expect("failed to create video");
                cx.new(|_| WithControlsExample::new(video))
            },
        );
        cx.activate(true);
    });
}
