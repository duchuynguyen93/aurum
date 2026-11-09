use std::ops::Range;

use gpui_kit::prelude::*;
use gpui_kit::*;

use super::format::clock;
use super::widgets::{icon, view_title};
use crate::state::app_state::AppState;
use crate::theme;

const ROW_HEIGHT: f32 = 44.;

impl AppState {
    pub(super) fn render_tracks(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.tracks.len();
        let subtitle = match (&self.scan, &self.library_error) {
            (_, Some(e)) => e.clone(),
            (Some(_), None) => format!("{count} songs, scanning"),
            (None, None) => format!("{count} songs"),
        };

        let body = if count == 0 {
            let text = if self.scan.is_some() || !self.library_loaded {
                "Reading your music..."
            } else {
                "No audio files were found in your folders."
            };
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(15.))
                .text_color(rgb(theme::IVORY_500))
                .child(text)
                .into_any_element()
        } else {
            uniform_list(
                "tracks",
                count,
                cx.processor(|this, range: Range<usize>, _, cx| {
                    range.map(|ix| this.track_row(ix, cx)).collect::<Vec<_>>()
                }),
            )
            .track_scroll(&self.tracks_scroll)
            .flex_1()
            .into_any_element()
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .px(px(32.))
            .pt(px(32.))
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .gap(px(12.))
                    .child(view_title("Tracks"))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(if self.library_error.is_some() {
                                theme::DANGER
                            } else {
                                theme::IVORY_500
                            }))
                            .child(subtitle),
                    ),
            )
            .child(div().flex_1().min_h_0().flex().flex_col().child(body))
    }

    fn track_row(&self, ix: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = &self.tracks[ix];
        let playing = self
            .playback
            .current
            .as_ref()
            .is_some_and(|c| c.path == t.path);
        let title_color = if playing {
            theme::GOLD_500
        } else {
            theme::IVORY_100
        };
        let number = t
            .track_no
            .map_or_else(|| (ix + 1).to_string(), |n| n.to_string());

        div()
            .id(("track", ix))
            .w_full()
            .h(px(ROW_HEIGHT))
            .px(px(12.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .rounded(px(10.))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::OBSIDIAN_800)))
            .on_click(cx.listener(move |s, e: &ClickEvent, _, cx| {
                if e.click_count() >= 2 {
                    s.play_library_track(ix, cx);
                }
            }))
            .child(
                div()
                    .w(px(28.))
                    .flex_none()
                    .flex()
                    .justify_end()
                    .font_family(theme::FONT_MONO)
                    .text_size(px(12.))
                    .text_color(rgb(theme::IVORY_500))
                    .child(if playing {
                        icon("volume-2", 16., theme::GOLD_500).into_any_element()
                    } else {
                        number.into_any_element()
                    }),
            )
            .child(
                div()
                    .w(relative(0.38))
                    .flex_none()
                    .truncate()
                    .text_size(px(15.))
                    .text_color(rgb(title_color))
                    .child(t.title.clone()),
            )
            .child(
                div()
                    .w(relative(0.24))
                    .flex_none()
                    .truncate()
                    .text_color(rgb(theme::IVORY_300))
                    .child(t.artist.clone().unwrap_or_default()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(theme::IVORY_500))
                    .child(t.album.clone().unwrap_or_default()),
            )
            .child(
                div()
                    .w(px(56.))
                    .flex_none()
                    .flex()
                    .justify_end()
                    .font_family(theme::FONT_MONO)
                    .text_size(px(12.))
                    .text_color(rgb(theme::IVORY_500))
                    .child(clock(std::time::Duration::from_millis(t.duration_ms))),
            )
    }
}
