use gpui::{App, Application, Context, Entity, Render, Window, WindowOptions, div, prelude::*};
use gpui_video_player::advanced_gpu_renderer;
use std::path::PathBuf;
use url::Url;

struct PlayerExample {
    video_renderer: Entity<gpui_video_player::AdvancedGpuRenderer>,
}

impl PlayerExample {
    fn new(video_renderer: Entity<gpui_video_player::AdvancedGpuRenderer>) -> Self {
        Self { video_renderer }
    }
}

impl Render for PlayerExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.video_renderer.clone())
    }
}

fn main() {
    env_logger::init();
    Application::new().run(|cx: &mut App| {
        let uri = Url::from_file_path(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("./assets/test.mp4"),
        )
        .expect("invalid file path");

        cx.open_window(
            WindowOptions {
                focus: true,
                ..Default::default()
            },
            |_, cx| {
                let video = gpui_video_player::Video::new(&uri).expect("failed to create video");
                let renderer = advanced_gpu_renderer(video);
                let renderer_entity = cx.new(|_| renderer);
                cx.new(|_| PlayerExample::new(renderer_entity))
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
