use gpui_kit::prelude::*;
use gpui_kit::*;

use super::widgets::{icon, secondary_button, view_title};
use crate::state::app_state::AppState;
use crate::theme;

fn section_label(text: &str) -> Div {
    div()
        .text_size(px(11.))
        .text_color(rgb(theme::IVORY_500))
        .child(text.to_uppercase())
}

impl AppState {
    pub(super) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let roots = self.settings.roots.iter().enumerate().map(|(i, root)| {
            div()
                .h(px(44.))
                .px(px(14.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .rounded(px(10.))
                .bg(rgb(theme::OBSIDIAN_850))
                .child(icon("folder-plus", 16., theme::IVORY_500))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(rgb(theme::IVORY_100))
                        .child(root.display().to_string()),
                )
                .child(
                    div()
                        .id(("root-remove", i))
                        .size(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(theme::OBSIDIAN_700)))
                        .on_click(cx.listener(move |s, _, _, cx| s.remove_root(i, cx)))
                        .child(icon("trash", 16., theme::IVORY_300)),
                )
        });

        let scanning = self.scan.is_some();
        let status = if let Some(p) = &self.scan {
            let ratio = if p.total_hint > 0 {
                (p.scanned as f32 / p.total_hint as f32).clamp(0., 1.)
            } else {
                0.
            };
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(div().text_color(rgb(theme::IVORY_300)).child(format!(
                    "Scanning: {} of {} files, {} added",
                    p.scanned, p.total_hint, p.added
                )))
                .child(
                    div()
                        .relative()
                        .w_full()
                        .h(px(4.))
                        .rounded_full()
                        .bg(rgb(theme::OBSIDIAN_700))
                        .child(
                            div()
                                .h_full()
                                .w(relative(ratio))
                                .rounded_full()
                                .bg(rgb(theme::GOLD_500)),
                        ),
                )
        } else {
            let summary = match &self.last_scan {
                Some(s) => format!(
                    "{} tracks, {} albums. Last scan: {} files, {} added, {} removed, {} errors.",
                    self.tracks.len(),
                    self.albums.len(),
                    s.scanned,
                    s.added,
                    s.removed,
                    s.errors
                ),
                None => format!(
                    "{} tracks, {} albums.",
                    self.tracks.len(),
                    self.albums.len()
                ),
            };
            div().text_color(rgb(theme::IVORY_300)).child(summary)
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .px(px(32.))
            .pt(px(32.))
            .gap(px(20.))
            .child(view_title("Settings"))
            .child(section_label("Music folders"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .max_w(px(720.))
                    .children(roots)
                    .when(self.settings.roots.is_empty(), |d| {
                        d.child(
                            div()
                                .text_color(rgb(theme::IVORY_500))
                                .child("No folders yet."),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(12.))
                    .child(secondary_button(
                        "settings-add",
                        "Add folder",
                        "folder-plus",
                        cx.listener(|s, _, _, cx| s.prompt_add_folder(cx)),
                    ))
                    .child(
                        secondary_button(
                            "settings-rescan",
                            "Rescan",
                            "refresh-cw",
                            cx.listener(|s, _, _, cx| s.start_scan(cx)),
                        )
                        .when(scanning, |b| b.opacity(0.5)),
                    ),
            )
            .child(div().max_w(px(720.)).child(status))
            .when_some(self.engine_error.clone(), |d, msg| {
                d.child(
                    div()
                        .max_w(px(720.))
                        .text_color(rgb(theme::DANGER))
                        .child(msg),
                )
            })
    }
}
