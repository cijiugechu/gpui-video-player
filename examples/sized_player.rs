use gpui::{
    App, Application, Context, Entity, EventEmitter, Render, Window, WindowOptions, actions, div,
    prelude::*, px, rgb,
};
use gpui_video_player::{ContentFit, VideoPlayer, VideoPlayerView, video_player};
use std::path::PathBuf;
use url::Url;

actions!(player_actions, [ToggleFit, ChangeSize]);

struct SizedPlayerExample {
    video_player: Entity<VideoPlayerView>,
    current_fit: ContentFit,
    current_size: (Option<f32>, Option<f32>), // (width, height)
}

impl SizedPlayerExample {
    fn new(video_player: Entity<VideoPlayerView>) -> Self {
        Self {
            video_player,
            current_fit: ContentFit::Contain,
            current_size: (Some(800.0), Some(600.0)),
        }
    }

    fn update_player(&mut self, cx: &mut Context<Self>) {
        let (width, height) = self.current_size;

        self.video_player.update(cx, |player_view, cx| {
            // Create a new player with updated settings
            let video = player_view.player().video().clone();
            let mut new_player = VideoPlayer::from_video(video).content_fit(self.current_fit);

            if let Some(w) = width {
                new_player = new_player.width(px(w));
            }
            if let Some(h) = height {
                new_player = new_player.height(px(h));
            }

            // Update the player
            *player_view = video_player(new_player);
            cx.notify();
        });
    }
}

impl EventEmitter<()> for SizedPlayerExample {}

impl Render for SizedPlayerExample {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fit_name = match self.current_fit {
            ContentFit::Contain => "Contain",
            ContentFit::Cover => "Cover",
            ContentFit::Fill => "Fill",
            ContentFit::ScaleDown => "ScaleDown",
            ContentFit::None => "None",
        };

        let size_text = match self.current_size {
            (Some(w), Some(h)) => format!("{}x{}", w, h),
            (Some(w), None) => format!("{}x?", w),
            (None, Some(h)) => format!("?x{}", h),
            (None, None) => "Natural".to_string(),
        };

        div()
            .size_full()
            .bg(rgb(0x2d2d2d))
            .flex()
            .flex_col()
            .child(
                // Control panel
                div()
                    .h(px(60.0))
                    .w_full()
                    .bg(rgb(0x1e1e1e))
                    .flex()
                    .items_center()
                    .px_4()
                    .gap_4()
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .bg(rgb(0x404040))
                            .rounded_md()
                            .cursor_pointer()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.current_fit = match this.current_fit {
                                        ContentFit::Contain => ContentFit::Cover,
                                        ContentFit::Cover => ContentFit::Fill,
                                        ContentFit::Fill => ContentFit::ScaleDown,
                                        ContentFit::ScaleDown => ContentFit::None,
                                        ContentFit::None => ContentFit::Contain,
                                    };
                                    this.update_player(cx);
                                }),
                            )
                            .child(format!("Fit: {}", fit_name)),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .bg(rgb(0x404040))
                            .rounded_md()
                            .cursor_pointer()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.current_size = match this.current_size {
                                        (Some(800.0), Some(600.0)) => (Some(400.0), Some(300.0)),
                                        (Some(400.0), Some(300.0)) => (Some(1200.0), Some(800.0)),
                                        (Some(1200.0), Some(800.0)) => (Some(600.0), None),
                                        (Some(600.0), None) => (None, Some(400.0)),
                                        (None, Some(400.0)) => (None, None),
                                        _ => (Some(800.0), Some(600.0)),
                                    };
                                    this.update_player(cx);
                                }),
                            )
                            .child(format!("Size: {}", size_text)),
                    )
                    .child(
                        div()
                            .text_color(rgb(0xcccccc))
                            .child("Click buttons to change video fit and size"),
                    ),
            )
            .child(
                // Video container
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(self.video_player.clone()),
            )
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
                let player = VideoPlayer::new(&uri)
                    .expect("failed to create video player")
                    .width(px(800.0))
                    .height(px(600.0))
                    .content_fit(ContentFit::Contain);

                let player_view = video_player(player);
                let player_entity = cx.new(|_| player_view);
                cx.new(|_| SizedPlayerExample::new(player_entity))
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
