//! The panel drawn with GPUI.

pub mod deck;
pub mod dialogs;
pub mod icons;
pub mod library;
pub mod mixer;
pub mod recent_images;
pub mod text_input;
pub mod theme;
pub mod tile;
pub mod widgets;

#[cfg(test)]
mod tests;

use gpui::{App, KeyBinding, actions};

actions!(deck, [FocusNext, FocusPrev]);

/// Tab and Shift+Tab move between the buttons; the text fields' keys.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", FocusNext, None),
        KeyBinding::new("shift-tab", FocusPrev, None),
    ]);
    text_input::bind_keys(cx);
}
