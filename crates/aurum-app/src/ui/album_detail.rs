//! Album page: blurred header with cover and actions, then the track list.

use std::rc::Rc;
use std::time::Duration;

use gpui_kit::prelude::*;
use gpui_kit::*;

use super::format::clock;
use super::widgets::{back_button, cover_fill, cover_shadow, gold_button, icon, outline_button};
use crate::state::app_state::{AlbumDetail, AppState};
use crate::state::cover_cache::Lookup;
use crate::theme;

const HEADER_PAD: f32 = 32.;
const COVER: f32 = 240.;

/// Total length as "12 min" below an hour, else "1 hr 15 min". Rounds up to whole minutes.
fn long_duration(d: Duration) -> String {
    let mins = d.as_secs().div_ceil(60);
    if mins >= 60 {
        format!("{} hr {} min", mins / 60, mins % 60)
    } else {
        format!("{mins} min")
    }
}

impl AppState {
    pub(super) fn render_album_detail(
        &mut self,
        d: Rc<AlbumDetail>,
        cx: &mut Context<Self>,
    ) -> Div {
        let lookup = self.covers.lookup(&d.cover_key());
        let blur = match &lookup {
            Lookup::Ready(art) => Some(art.blur.clone()),
            _ => None,
        };

        let meta = [
            d.year.map(|y| y.to_string()),
            Some(format!(
                "{} {}",
                d.tracks.len(),
                if d.tracks.len() == 1 {
                    "track"
                } else {
                    "tracks"
                }
            )),
            Some(long_duration(d.total)),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" \u{b7} ");

        let play_d = d.clone();
        let shuffle_d = d.clone();
        let actions = div()
            .mt(px(20.))
            .flex()
            .flex_row()
            .gap(px(12.))
            .child(gold_button("album-play", "Play", "play").on_click(
                cx.listener(move |s, _, _, cx| s.play_tracks(play_d.tracks.clone(), 0, cx)),
            ))
            .child(
                outline_button("album-shuffle", "Shuffle", "shuffle").on_click(
                    cx.listener(move |s, _, _, cx| s.play_shuffled(shuffle_d.tracks.clone(), cx)),
                ),
            );

        let text = div()
            .flex_1()
            .min_w_0()
            .h(px(COVER))
            .flex()
            .flex_col()
            .justify_end()
            .child(
                div()
                    .font_family(theme::FONT_DISPLAY)
                    .text_size(px(34.))
                    .line_height(px(40.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::IVORY_50))
                    .line_clamp(2)
                    .child(d.name.clone()),
            )
            .child(
                div()
                    .mt(px(6.))
                    .text_size(px(15.))
                    .text_color(rgb(theme::IVORY_100))
                    .truncate()
                    .child(d.artist.clone().unwrap_or_else(|| "Unknown artist".into())),
            )
            .child(
                div()
                    .mt(px(6.))
                    .font_family(theme::FONT_MONO)
                    .text_size(px(12.))
                    .text_color(rgb(theme::IVORY_300))
                    .child(meta),
            )
            .child(actions);

        let header =
            div()
                .relative()
                .w_full()
                .flex_none()
                .overflow_hidden()
                .children(blur.map(|image| {
                    img(image)
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                }))
                // Dims the blurred cover to 30% and fades the header into the page background.
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .bg(theme::color(theme::OBSIDIAN_950).opacity(0.7)),
                )
                .child(div().absolute().left_0().bottom_0().w_full().h(px(96.)).bg(
                    linear_gradient(
                        180.,
                        linear_color_stop(theme::color(theme::OBSIDIAN_950).opacity(0.), 0.),
                        linear_color_stop(theme::color(theme::OBSIDIAN_950), 1.),
                    ),
                ))
                .child(
                    div()
                        .relative()
                        .px(px(HEADER_PAD))
                        .pt(px(48.))
                        .pb(px(32.))
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap(px(20.))
                        .child(back_button(cx))
                        .child(
                            div()
                                .size(px(COVER))
                                .flex_none()
                                .rounded(px(20.))
                                .overflow_hidden()
                                .bg(rgb(theme::OBSIDIAN_800))
                                .shadow(cover_shadow(1.0))
                                .child(cover_fill(lookup, 64.)),
                        )
                        .child(div().w(px(12.)).flex_none())
                        .child(text),
                );

        let multi_disc = d.disc_count() > 1;
        let mut rows = Vec::new();
        let mut last_disc = None;
        for (ix, t) in d.tracks.iter().enumerate() {
            let disc = t.disc_no.unwrap_or(1);
            if multi_disc && last_disc != Some(disc) {
                last_disc = Some(disc);
                rows.push(
                    div()
                        .h(px(40.))
                        .px(px(12.))
                        .pt(px(8.))
                        .flex()
                        .items_end()
                        .text_size(px(11.))
                        .text_color(rgb(theme::IVORY_500))
                        .child(format!("DISC {disc}"))
                        .into_any_element(),
                );
            }
            rows.push(self.album_track_row(&d, ix, cx).into_any_element());
        }

        let list = div()
            .id("album-tracks")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&d.scroll)
            .px(px(HEADER_PAD))
            .pb(px(24.))
            .flex()
            .flex_col()
            .children(rows);

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(header)
            .child(list)
    }

    fn album_track_row(
        &self,
        d: &Rc<AlbumDetail>,
        ix: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = &d.tracks[ix];
        let playing = self
            .playback
            .current
            .as_ref()
            .is_some_and(|c| c.path == t.path);
        let number = t
            .track_no
            .map_or_else(|| (ix + 1).to_string(), |n| n.to_string());
        // Show the track artist only when it differs from the album artist (compilations).
        let artist = t
            .artist
            .clone()
            .filter(|a| Some(a) != d.artist.as_ref())
            .unwrap_or_default();
        let play_d = d.clone();

        div()
            .id(("album-track", ix))
            .w_full()
            .h(px(44.))
            .flex_none()
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
                    s.play_tracks(play_d.tracks.clone(), ix, cx);
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
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(15.))
                    .text_color(rgb(if playing {
                        theme::GOLD_500
                    } else {
                        theme::IVORY_100
                    }))
                    .child(t.title.clone()),
            )
            .child(
                div()
                    .max_w(relative(0.3))
                    .flex_none()
                    .truncate()
                    .text_color(rgb(theme::IVORY_500))
                    .child(artist),
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
                    .child(clock(Duration::from_millis(t.duration_ms))),
            )
    }
}

#[cfg(test)]
mod tests {
    // Not `use super::*`: that would import gpui's own `test` attribute over `#[test]`.
    use std::time::Duration;

    use super::long_duration;

    #[test]
    fn long_duration_rounds_up_to_minutes() {
        assert_eq!(long_duration(Duration::from_secs(61)), "2 min");
        assert_eq!(long_duration(Duration::from_secs(3600)), "1 hr 0 min");
        assert_eq!(long_duration(Duration::from_secs(4500)), "1 hr 15 min");
    }
}
