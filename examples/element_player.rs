use gpui::{App, Application, Context, Render, Window, WindowOptions, div, prelude::*};
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
            .child(video(self.video.clone()).id("main-video"))
    }
}

fn main() {
    env_logger::init();
    Application::new().run(|cx: &mut App| {
        let uri = Url::from_file_path(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("./assets/test2.mp4"),
        )
        .expect("invalid file path");

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
