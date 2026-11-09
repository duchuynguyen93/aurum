mod theme;

use gpui_kit::*;

struct AurumWindow;

impl AurumWindow {
    /// GPUI has no letter-spacing, so wide tracking is emulated with one child per letter.
    fn wordmark(text: &str) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .gap(px(5.))
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::GOLD_500))
            .children(text.chars().map(|c| div().child(c.to_string())))
    }
}

impl Render for AurumWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(rgb(theme::OBSIDIAN_950))
            .text_color(rgb(theme::IVORY_100))
            .child(
                div()
                    .w(px(220.))
                    .h_full()
                    .flex_none()
                    .bg(rgb(theme::OBSIDIAN_900))
                    .p(px(24.))
                    .child(Self::wordmark("AURUM")),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .p(px(32.))
                    .child(
                        div()
                            .text_size(px(22.))
                            .text_color(rgb(theme::IVORY_50))
                            .child("Albums"),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(theme::IVORY_300))
                            .child("Library is empty"),
                    ),
            )
    }
}

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        let bounds = Bounds::centered(None, size(px(1200.), px(760.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Aurum".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| AurumWindow))
            .expect("failed to open window");
        cx.activate(true);
    });
}
