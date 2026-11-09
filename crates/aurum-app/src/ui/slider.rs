//! Horizontal slider drawn by hand: 2px track, gold fill, 10px handle shown on hover or drag.
//! Click and drag both report a ratio in `0.0..=1.0`.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme;

/// Empty view GPUI needs to render the dragged value; the slider has no drag ghost.
struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

type ChangeFn = Rc<dyn Fn(f32, &mut Window, &mut App)>;
type ReleaseFn = Rc<dyn Fn(&mut Window, &mut App)>;

pub struct Slider<D> {
    pub id: &'static str,
    /// Distinct per slider: GPUI routes drag-move events by the dragged value's type.
    pub drag: D,
    pub ratio: f32,
    /// Keep the handle visible (while dragging).
    pub active: bool,
    pub bounds: Rc<Cell<Bounds<Pixels>>>,
    pub on_change: ChangeFn,
    pub on_release: ReleaseFn,
}

fn ratio_at(bounds: &Bounds<Pixels>, x: Pixels) -> f32 {
    if bounds.size.width <= px(0.) {
        return 0.;
    }
    ((x - bounds.origin.x) / bounds.size.width).clamp(0., 1.)
}

impl<D: 'static> Slider<D> {
    pub fn render(self) -> Stateful<Div> {
        let ratio = self.ratio.clamp(0., 1.);
        let group: SharedString = format!("slider-{}", self.id).into();

        let cell = self.bounds.clone();
        let measure = canvas(move |b, _, _| cell.set(b), |_, _, _, _| {})
            .absolute()
            .size_full();

        let handle = div()
            .absolute()
            .left(relative(ratio))
            .top(px(-4.))
            .ml(px(-5.))
            .size(px(10.))
            .rounded_full()
            .bg(rgb(theme::IVORY_50))
            .opacity(if self.active { 1. } else { 0. })
            .group_hover(group.clone(), |s| s.opacity(1.));

        let track = div()
            .relative()
            .w_full()
            .h(px(2.))
            .rounded_full()
            .bg(rgb(theme::OBSIDIAN_700))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(relative(ratio))
                    .rounded_full()
                    .bg(rgb(theme::GOLD_500)),
            )
            .child(handle);

        let (bounds_down, bounds_drag) = (self.bounds.clone(), self.bounds.clone());
        let (change_down, change_drag) = (self.on_change.clone(), self.on_change);
        let (release_up, release_out) = (self.on_release.clone(), self.on_release);

        div()
            .id(self.id)
            .group(group)
            .relative()
            .w_full()
            .h(px(16.))
            .flex()
            .items_center()
            .cursor_pointer()
            .child(measure)
            .child(track)
            .on_mouse_down(MouseButton::Left, move |e, w, cx| {
                change_down(ratio_at(&bounds_down.get(), e.position.x), w, cx);
            })
            .on_drag(self.drag, |_, _, _, cx| cx.new(|_| DragGhost))
            .on_drag_move::<D>(move |e, w, cx| {
                change_drag(ratio_at(&bounds_drag.get(), e.event.position.x), w, cx);
            })
            .on_mouse_up(MouseButton::Left, move |_, w, cx| release_up(w, cx))
            .on_mouse_up_out(MouseButton::Left, move |_, w, cx| release_out(w, cx))
    }
}
