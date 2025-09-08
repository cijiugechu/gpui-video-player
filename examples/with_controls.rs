use gpui::{
    App, Application, Context, CursorStyle, Render, Window, WindowOptions, div, prelude::*,
};
use gpui_component::button::Button;
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
        let play_label = if is_paused { "▶️" } else { "⏸️" };

        let back_5s = Button::new("back-5s")
            .label("⏪")
            .cursor(CursorStyle::PointingHand)
            .on_click(cx.listener(|this: &mut Self, _event, _window, cx| {
                if !this.click_allowed() {
                    return;
                }
                let pos = this.video.position();
                let new_pos = pos.saturating_sub(Duration::from_secs(5));

                cx.spawn(async move |handle, cx| {
                    handle
                        .update(cx, |this, cx| {
                            let _ = this.video.seek(new_pos, false);
                            cx.notify();
                        })
                        .ok();
                })
                .detach();
            }));

        let play_pause = Button::new("play-pause")
            .label(play_label)
            .cursor(CursorStyle::PointingHand)
            .on_click(cx.listener(|this: &mut Self, _event, _window, cx| {
                if !this.click_allowed() {
                    return;
                }
                let paused = this.video.paused();
                this.video.set_paused(!paused);
                cx.notify();
            }));

        let forward_5s = Button::new("forward-5s")
            .label("⏩")
            .cursor(CursorStyle::PointingHand)
            .on_click(cx.listener(|this: &mut Self, _event, _window, cx| {
                if !this.click_allowed() {
                    return;
                }
                let pos = this.video.position();
                let dur = this.video.duration();
                let target = pos.saturating_add(Duration::from_secs(5));
                let new_pos = if target > dur { dur } else { target };

                cx.spawn(async move |handle, cx| {
                    handle
                        .update(cx, |this, cx| {
                            let _ = this.video.seek(new_pos, false);
                            cx.notify();
                        })
                        .ok();
                })
                .detach();
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
                gpui_component::init(cx);

                let video = Video::new(&uri).expect("failed to create video");
                cx.new(|_| WithControlsExample::new(video))
            },
        );
        cx.activate(true);
    });
}
