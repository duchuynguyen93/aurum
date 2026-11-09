//! Albums grid (also used by the artist page) and the Artists list.

use std::ops::Range;

use gpui_kit::prelude::*;
use gpui_kit::*;

use super::widgets::{back_button, cover_fill, cover_shadow, view_title};
use crate::state::app_state::AppState;
use crate::theme;

/// Horizontal padding of the grid rows, and the free space around the grid.
const GRID_PAD: f32 = 32.;
const GRID_GAP: f32 = 24.;
const CARD_MIN: f32 = 160.;
/// Space between the cover and the text, and the fixed height of the three text lines.
const CARD_TEXT_GAP: f32 = 12.;
const CARD_TEXT_H: f32 = 20. + 18. + 16.;
pub(super) const SIDEBAR_WIDTH: f32 = 220.;

/// `(columns, card width)` for a window `window_width` px wide: as many cards of at least
/// `CARD_MIN` as fit, stretched to fill the row.
fn grid_metrics(window_width: f32) -> (usize, f32) {
    let avail = (window_width - SIDEBAR_WIDTH - 2. * GRID_PAD).max(CARD_MIN);
    let cols = (((avail + GRID_GAP) / (CARD_MIN + GRID_GAP)).floor() as usize).max(1);
    let card = (avail - GRID_GAP * (cols - 1) as f32) / cols as f32;
    (cols, card)
}

fn centered_note(text: String) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(15.))
        .text_color(rgb(theme::IVORY_500))
        .child(text)
        .into_any_element()
}

impl AppState {
    fn empty_note(&self, what: &str) -> String {
        if !self.search.is_empty() {
            format!("No {what} match \u{201c}{}\u{201d}.", self.search)
        } else if self.scan.is_some() || !self.library_loaded {
            "Reading your music...".to_string()
        } else {
            format!("No {what} found in your folders.")
        }
    }

    pub(super) fn render_albums(&mut self, window: &Window, cx: &mut Context<Self>) -> Div {
        let count = self.shown_albums.len();
        let subtitle = match &self.library_error {
            Some(e) => e.clone(),
            None => format!("{count} albums"),
        };
        let left = div()
            .flex()
            .flex_row()
            .items_baseline()
            .gap(px(12.))
            .child(view_title("Albums"))
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(rgb(if self.library_error.is_some() {
                        theme::DANGER
                    } else {
                        theme::IVORY_500
                    }))
                    .child(subtitle),
            );
        let scroll = self.albums_scroll.clone();
        let grid = self.album_grid("albums", scroll, window, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .pt(px(48.))
            .gap(px(16.))
            .child(self.view_header(left).px(px(GRID_PAD)))
            .child(grid)
    }

    pub(super) fn render_artist_detail(
        &mut self,
        name: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let (albums, tracks) = self
            .artists
            .iter()
            .find(|a| a.name == name)
            .map_or((0, 0), |a| (a.albums, a.tracks));
        let left = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(16.))
            .child(back_button(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .font_family(theme::FONT_DISPLAY)
                            .text_size(px(34.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::IVORY_50))
                            .child(name.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(theme::IVORY_300))
                            .child(format!("{albums} albums \u{b7} {tracks} tracks")),
                    ),
            );
        let scroll = self.artist_albums_scroll.clone();
        let grid = self.album_grid("artist-albums", scroll, window, cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .pt(px(48.))
            .gap(px(24.))
            .child(self.view_header(left).px(px(GRID_PAD)))
            .child(grid)
    }

    /// Virtualised grid: one `uniform_list` item per row of cards. The column count follows the
    /// window width, so the row height (card plus text) is the same for every row.
    fn album_grid(
        &mut self,
        id: &'static str,
        scroll: UniformListScrollHandle,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let count = self.grid_albums().len();
        if count == 0 {
            return centered_note(self.empty_note("albums"));
        }
        let (cols, card_w) = grid_metrics(f32::from(window.viewport_size().width));
        uniform_list(
            id,
            count.div_ceil(cols),
            cx.processor(move |this, range: Range<usize>, _, cx| {
                let albums = this.grid_albums();
                range
                    .map(|row| this.album_row(&albums, row, cols, card_w, cx))
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&scroll)
        .flex_1()
        .min_h_0()
        .into_any_element()
    }

    fn album_row(
        &mut self,
        albums: &[usize],
        row: usize,
        cols: usize,
        card_w: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let cards: Vec<_> = albums
            .iter()
            .skip(row * cols)
            .take(cols)
            .map(|&ix| self.album_card(ix, card_w, cx))
            .collect();
        div()
            .w_full()
            .h(px(card_w + CARD_TEXT_GAP + CARD_TEXT_H + GRID_GAP))
            .px(px(GRID_PAD))
            .flex()
            .flex_row()
            .items_start()
            .gap(px(GRID_GAP))
            .children(cards)
    }

    fn album_card(
        &mut self,
        ix: usize,
        card_w: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let album = &self.albums[ix];
        let key = (album.artist.clone().unwrap_or_default(), album.name.clone());
        let name = album.name.clone();
        let artist = album
            .artist
            .clone()
            .unwrap_or_else(|| "Unknown artist".into());
        let year = album.year.map(|y| y.to_string()).unwrap_or_default();
        let path = album.first_track_path.clone();
        self.ensure_cover_for(key.clone(), path, cx);
        let lookup = self.covers.lookup(&key);

        div()
            .id(("album", ix))
            .w(px(card_w))
            .flex_none()
            .flex()
            .flex_col()
            .cursor_pointer()
            .on_click(cx.listener(move |s, _, _, cx| s.open_album(ix, cx)))
            .child(
                div()
                    .size(px(card_w))
                    .rounded(px(14.))
                    .overflow_hidden()
                    .bg(rgb(theme::OBSIDIAN_800))
                    .border_1()
                    .border_color(theme::hairline())
                    .hover(|s| s.border_color(theme::color(theme::IVORY_50).opacity(0.28)))
                    .shadow(cover_shadow(0.4))
                    .child(cover_fill(lookup, 36.)),
            )
            .child(
                div()
                    .mt(px(CARD_TEXT_GAP))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(20.))
                            .truncate()
                            .font_family(theme::FONT_DISPLAY)
                            .text_size(px(15.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::IVORY_50))
                            .child(name),
                    )
                    .child(
                        div()
                            .h(px(18.))
                            .truncate()
                            .text_size(px(13.))
                            .text_color(rgb(theme::IVORY_300))
                            .child(artist),
                    )
                    .child(
                        div()
                            .h(px(16.))
                            .truncate()
                            .text_size(px(11.))
                            .text_color(rgb(theme::IVORY_500))
                            .child(year),
                    ),
            )
    }

    pub(super) fn render_artists(&mut self, cx: &mut Context<Self>) -> Div {
        let count = self.shown_artists.len();
        let left = div()
            .flex()
            .flex_row()
            .items_baseline()
            .gap(px(12.))
            .child(view_title("Artists"))
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(rgb(theme::IVORY_500))
                    .child(format!("{count} artists")),
            );
        let body = if count == 0 {
            centered_note(self.empty_note("artists"))
        } else {
            uniform_list(
                "artists",
                count,
                cx.processor(|this, range: Range<usize>, _, cx| {
                    range.map(|i| this.artist_row(i, cx)).collect::<Vec<_>>()
                }),
            )
            .track_scroll(&self.artists_scroll)
            .flex_1()
            .min_h_0()
            .into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .pt(px(48.))
            .gap(px(16.))
            .child(self.view_header(left).px(px(GRID_PAD)))
            .child(body)
    }

    fn artist_row(&self, i: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let artist = &self.artists[self.shown_artists[i]];
        let name = artist.name.clone();
        let open_name = artist.name.clone();
        let counts = format!(
            "{} {} \u{b7} {} {}",
            artist.albums,
            if artist.albums == 1 {
                "album"
            } else {
                "albums"
            },
            artist.tracks,
            if artist.tracks == 1 {
                "track"
            } else {
                "tracks"
            },
        );
        // The row keeps the list's side padding inside its own box so the hover fill is inset.
        div().w_full().h(px(56.)).px(px(GRID_PAD - 12.)).child(
            div()
                .id(("artist", i))
                .size_full()
                .px(px(12.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(16.))
                .rounded(px(10.))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(theme::OBSIDIAN_800)))
                .on_click(cx.listener(move |s, _, _, cx| s.open_artist(open_name.clone(), cx)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(15.))
                        .text_color(rgb(theme::IVORY_100))
                        .child(name),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(13.))
                        .text_color(rgb(theme::IVORY_300))
                        .child(counts),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{CARD_MIN, GRID_GAP, grid_metrics};

    #[test]
    fn grid_fits_whole_cards_and_never_underflows() {
        // 1200 wide: 1200 - 220 - 64 = 916 free, 5 cards of 160 + 4 gaps of 24 = 896 fit.
        let (cols, card) = grid_metrics(1200.);
        assert_eq!(cols, 5);
        assert!(card >= CARD_MIN);
        assert!((card * cols as f32 + GRID_GAP * (cols - 1) as f32 - 916.).abs() < 0.01);
        // A tiny window still gives one column of at least the minimum width.
        assert_eq!(grid_metrics(100.).0, 1);
    }
}
