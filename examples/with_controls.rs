use gpui_video_player::gpui::{
    App, Context, CursorStyle, FocusHandle, KeyDownEvent, Render, Window, WindowOptions, div,
    prelude::*,
};
use gpui_video_player::{Video, video};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use url::Url;

#[derive(Clone, Copy)]
enum Control {
    Back,
    PlayPause,
    Forward,
}

struct WithControlsExample {
    video: Video,
    last_click: Option<Instant>,
    error: Option<String>,
    focus_handle: FocusHandle,
}

impl WithControlsExample {
    fn new(video: Video, cx: &mut Context<Self>) -> Self {
        Self {
            video,
            last_click: None,
            error: None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn click_allowed(&mut self) -> bool {
        let now = Instant::now();
        if let Some(prev) = self.last_click
            && now.saturating_duration_since(prev) < Duration::from_millis(250)
        {
            return false;
        }
        self.last_click = Some(now);
        true
    }

    fn activate(&mut self, control: Control, cx: &mut Context<Self>) {
        if !self.click_allowed() {
            return;
        }
        self.error = None;
        let result = match control {
            Control::Back => self.video.seek(
                self.video.position().saturating_sub(Duration::from_secs(5)),
                false,
            ),
            Control::Forward => self.video.seek(
                self.video
                    .position()
                    .saturating_add(Duration::from_secs(5))
                    .min(self.video.duration()),
                false,
            ),
            Control::PlayPause => {
                self.video.set_paused(!self.video.paused());
                Ok(())
            }
        };
        if let Err(error) = result {
            log::error!("playback control failed: {error}");
            self.error = Some(error.to_string());
        }
        cx.notify();
    }

    fn control(
        &self,
        id: &'static str,
        label: &'static str,
        control: Control,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div()
            .id(id)
            .focusable()
            .tab_stop(true)
            .px_3()
            .py_1()
            .rounded_md()
            .bg(gpui_video_player::gpui::rgba(0x000000aa))
            .text_color(gpui_video_player::gpui::white())
            .border_1()
            .border_color(gpui_video_player::gpui::rgba(0xffffff44))
            .hover(|style| style.bg(gpui_video_player::gpui::rgba(0x333333cc)))
            .focus(|style| style.border_color(gpui_video_player::gpui::rgb(0x4488ff)))
            .child(label)
            .cursor(CursorStyle::PointingHand)
            .on_click(cx.listener(move |this, _, _, cx| this.activate(control, cx)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    if !event.is_held {
                        this.activate(control, cx);
                    }
                }
            }))
    }
}

impl Render for WithControlsExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_paused = self.video.paused();
        let play_label = if is_paused { "Play" } else { "Pause" };

        let back_5s = self.control("back-5s", "Back 5s", Control::Back, cx);
        let play_pause = self.control("play-pause", play_label, Control::PlayPause, cx);
        let forward_5s = self.control("forward-5s", "Forward 5s", Control::Forward, cx);

        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .tab_stop(false)
            .on_key_down(|event, window, cx| {
                if event.keystroke.key == "tab" {
                    cx.stop_propagation();
                    #[cfg(feature = "gpui-pre")]
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                    #[cfg(not(feature = "gpui-pre"))]
                    if event.keystroke.modifiers.shift {
                        window.focus_prev();
                    } else {
                        window.focus_next();
                    }
                }
            })
            .flex()
            .flex_col()
            .when_some(self.error.clone(), |this, error| {
                this.child(
                    div()
                        .text_color(gpui_video_player::gpui::rgb(0xff5555))
                        .child(error),
                )
            })
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
                            .top_0()
                            .left_0()
                            .size_full()
                            .pt_3()
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
    #[cfg(feature = "gpui-pre")]
    let application = gpui_platform::application();
    #[cfg(not(feature = "gpui-pre"))]
    let application = gpui_video_player::gpui::Application::new();
    application.run(|cx: &mut App| {
        let uri = Url::from_file_path(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("./assets/test3.mp4"),
        )
        .expect("invalid file path");

        if let Err(error) = cx.open_window(
            WindowOptions {
                focus: true,
                ..Default::default()
            },
            |window, cx| {
                let video = Video::new(&uri).expect("failed to create video");
                cx.new(|cx| {
                    let view = WithControlsExample::new(video, cx);
                    #[cfg(feature = "gpui-pre")]
                    window.focus(&view.focus_handle, cx);
                    #[cfg(not(feature = "gpui-pre"))]
                    window.focus(&view.focus_handle);
                    view
                })
            },
        ) {
            log::error!("failed to open controls window: {error}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}
