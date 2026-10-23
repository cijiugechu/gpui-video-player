use gpui_video_player::gpui::{App, Context, Render, Window, WindowOptions, div, prelude::*};
use gpui_video_player::{Video, video};
use std::path::PathBuf;
use url::Url;

struct ElementPlayerExample {
    video: Video,
}

impl ElementPlayerExample {
    fn new(video: Video) -> Self {
        Self { video }
    }
}

impl Render for ElementPlayerExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                video(self.video.clone())
                    .id("main-video")
                    .buffer_capacity(30),
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

        if std::env::args().any(|arg| arg == "--stress") {
            cx.spawn(async move |cx| {
                for cycle in 0..20 {
                    let video = match Video::new(&uri) {
                        Ok(video) => video,
                        Err(error) => {
                            log::error!("stress playback failed: {error}");
                            quit_stress(cx);
                            return;
                        }
                    };
                    let window = match cx.open_window(WindowOptions::default(), |_, cx| {
                        cx.new(|_| ElementPlayerExample::new(video))
                    }) {
                        Ok(window) => window,
                        Err(error) => {
                            log::error!("stress window creation failed: {error}");
                            quit_stress(cx);
                            return;
                        }
                    };
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(1))
                        .await;
                    if let Err(error) = window.update(cx, |_, window, _| window.remove_window()) {
                        log::error!("stress window teardown failed: {error}");
                        quit_stress(cx);
                        return;
                    }
                    log::info!("stress window {}/20 closed", cycle + 1);
                }
                log::info!("window stress completed");
                quit_stress(cx);
            })
            .detach();
            return;
        }

        cx.open_window(
            WindowOptions {
                focus: true,
                ..Default::default()
            },
            |_, cx| {
                let video = Video::new(&uri).expect("failed to create video");
                cx.new(|_| ElementPlayerExample::new(video))
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

fn quit_stress(cx: &mut gpui_video_player::gpui::AsyncApp) {
    #[cfg(feature = "gpui-pre")]
    cx.update(|cx| cx.quit());
    #[cfg(not(feature = "gpui-pre"))]
    if let Err(error) = cx.update(|cx| cx.quit()) {
        log::error!("failed to quit stress application: {error}");
    }
}
