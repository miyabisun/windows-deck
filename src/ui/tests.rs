//! The panel against the windows-link stand-in, as a user drives it: the counterparts of the
//! Playwright suite (`e2e/*.spec.js` before GPUI).

mod deck;
mod desktops;
mod dlsite;
mod library;
mod mixer;

use std::{rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AppContext as _, Bounds, Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent,
    PlatformInput, Pixels, TestAppContext, TouchEvent, TouchId, TouchPhase, VisualTestContext, px,
    size,
};

use super::{deck::Deck, theme::Theme};
use crate::link::{
    Link,
    mock::{BASE, MockLink},
};

/// The panel in a 1920×1280 window, connected to `mock`, its monitor configured.
pub fn open<'a>(cx: &'a mut TestAppContext, mock: &MockLink) -> (Entity<Deck>, &'a mut VisualTestContext) {
    open_with(cx, mock, Some("T10FA"))
}

pub fn open_with<'a>(
    cx: &'a mut TestAppContext,
    mock: &MockLink,
    monitor: Option<&str>,
) -> (Entity<Deck>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        cx.set_global(Theme::sumi());
        super::bind_keys(cx);
    });
    let (mock, monitor) = (mock.clone(), monitor.map(str::to_owned));
    let (deck, vcx) = cx.add_window_view(move |window, cx| {
        let link = cx.new(|cx| {
            let mut link = Link::new(BASE, Arc::new(mock.clone()), Arc::new(mock.clone()), Arc::new(mock.clone()));
            link.start(cx);
            link
        });
        Deck::new(link, monitor, None, None, Some(4_723_016), Rc::new(|_, _| {}), window, cx)
    });
    vcx.simulate_resize(size(px(1920.), px(1280.)));
    vcx.run_until_parked();
    (deck, vcx)
}

fn leak(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

pub fn bounds(vcx: &mut VisualTestContext, selector: &str) -> Option<Bounds<Pixels>> {
    vcx.run_until_parked();
    vcx.debug_bounds(leak(selector.to_owned()))
}

/// Whether an element with this debug selector is on the screen.
pub fn has(vcx: &mut VisualTestContext, selector: &str) -> bool {
    bounds(vcx, selector).is_some()
}

/// Whether this text is on the screen.
pub fn shows(vcx: &mut VisualTestContext, text: &str) -> bool {
    has(vcx, &format!("text:{text}"))
}

fn centre(vcx: &mut VisualTestContext, selector: &str) -> gpui::Point<Pixels> {
    bounds(vcx, selector).unwrap_or_else(|| panic!("{selector} is not on the screen")).center()
}

pub fn click(vcx: &mut VisualTestContext, selector: &str) {
    let at = centre(vcx, selector);
    vcx.simulate_click(at, Modifiers::none());
    vcx.run_until_parked();
}

pub fn right_click(vcx: &mut VisualTestContext, selector: &str) {
    let position = centre(vcx, selector);
    vcx.simulate_event(MouseDownEvent { button: MouseButton::Right, position, modifiers: Modifiers::none(), click_count: 1, first_mouse: false });
    vcx.simulate_event(MouseUpEvent { button: MouseButton::Right, position, modifiers: Modifiers::none(), click_count: 1 });
    vcx.run_until_parked();
}

fn touch(vcx: &mut VisualTestContext, phase: TouchPhase, position: gpui::Point<Pixels>) {
    vcx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::Touch(TouchEvent { id: TouchId(1), phase, position, predicted_position: None, force: None }),
            cx,
        );
    });
}

/// A finger's tap, as the touch screen sends it through Windows' pointer messages.
pub fn tap(vcx: &mut VisualTestContext, selector: &str) {
    let at = centre(vcx, selector);
    touch(vcx, TouchPhase::Started, at);
    touch(vcx, TouchPhase::Ended, at);
    vcx.run_until_parked();
}

/// A finger held still for a long press.
pub fn long_press(vcx: &mut VisualTestContext, selector: &str) {
    let at = centre(vcx, selector);
    touch(vcx, TouchPhase::Started, at);
    vcx.executor().advance_clock(Duration::from_millis(600));
    vcx.run_until_parked();
    touch(vcx, TouchPhase::Ended, at);
    vcx.run_until_parked();
}

/// A finger sliding from the middle of `selector` by `dx`, `dy`, and resting there before it
/// lifts (so there is no fling to run on).
pub fn slide(vcx: &mut VisualTestContext, selector: &str, dx: f32, dy: f32) {
    let start = centre(vcx, selector);
    touch(vcx, TouchPhase::Started, start);
    for step in 1..=10 {
        #[allow(clippy::cast_precision_loss)]
        let part = step as f32 / 10.;
        let at = gpui::point(start.x + px(dx * part), start.y + px(dy * part));
        vcx.executor().advance_clock(Duration::from_millis(16));
        touch(vcx, TouchPhase::Moved, at);
    }
    let end = gpui::point(start.x + px(dx), start.y + px(dy));
    vcx.executor().advance_clock(Duration::from_millis(300));
    touch(vcx, TouchPhase::Moved, end);
    touch(vcx, TouchPhase::Ended, end);
    vcx.run_until_parked();
}

/// Keys pressed and released one after another, as a keyboard sends them ("tab right enter").
pub fn keys(vcx: &mut VisualTestContext, keystrokes: &str) {
    for stroke in keystrokes.split_whitespace() {
        let keystroke = gpui::Keystroke::parse(leak(stroke.to_owned())).expect("a keystroke");
        vcx.simulate_keystrokes(stroke);
        vcx.simulate_event(gpui::KeyUpEvent { keystroke });
        vcx.run_until_parked();
    }
}

/// The link the panel talks through.
pub fn link(deck: &Entity<Deck>, vcx: &mut VisualTestContext) -> Entity<Link> {
    deck.read_with(vcx, |deck, _| deck.link())
}
