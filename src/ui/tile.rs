//! A button of the panel, and the tile of a game (in a library list or pinned to a tab).

use std::{cell::Cell, rc::Rc};

use gpui::{
    App, FontWeight, IntoElement, ObjectFit, ParentElement, Pixels, Render, Role, SharedString,
    Styled, Window, div, img, prelude::*, px,
};

use super::{
    icons::{glyph, spinner},
    theme::{FS_LABEL, FS_SM, FS_STATE, RADIUS_MD, RADIUS_SM, SP_1, SP_2, SP_3, SP_4, SP_5, TILE_HEIGHT, Theme},
    widgets::{OnLong, is_menu_key, long_press},
};
use crate::model::display::{Glyph, Shown};

/// What a tile shows.
pub enum Face {
    /// A windows-link button: its picture or state icon, and its state.
    Button {
        shown: Shown,
        picture: Option<SharedString>,
        glyph: Option<Glyph>,
    },
    /// A game: its picture across the top, and a note under its name.
    Game {
        cover: Option<SharedString>,
        /// Shown whole (DLsite's art, program icons) rather than filling the tile.
        whole: bool,
        note: Option<String>,
    },
}

/// A game being dragged with the mouse onto a label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraggedGame {
    pub id: String,
    pub name: SharedString,
}

/// What follows the mouse while a game is dragged.
pub struct DragPreview(SharedString);

impl Render for DragPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        div()
            .px(SP_4)
            .py(SP_2)
            .rounded(RADIUS_MD)
            .border_1()
            .border_color(theme.accent)
            .bg(theme.surface_raised)
            .text_color(theme.on_surface)
            .text_size(FS_LABEL)
            .opacity(0.9)
            .child(self.0.clone())
    }
}

pub struct Tile {
    pub key: SharedString,
    pub label: SharedString,
    pub face: Face,
    pub pending: bool,
    pub failure: Option<String>,
    pub disabled: bool,
    /// Under a cover (resting, or a dialog): looks as usual but takes no input.
    pub inert: bool,
    pub on_press: Rc<dyn Fn(&mut Window, &mut App)>,
    pub on_long: Option<OnLong>,
    pub drag: Option<DraggedGame>,
    /// Pictures without colour, under the resting cover.
    pub gray: bool,
    /// The width the tile is laid out at, for the height of a game's picture.
    pub width: Pixels,
}

impl Tile {
    pub fn render(self, theme: &Theme) -> impl IntoElement + use<> {
        let game = matches!(self.face, Face::Game { .. });
        let (failed, note) = match &self.face {
            Face::Button { shown, .. } => (shown.failed, shown.note.clone()),
            Face::Game { note, .. } => (false, note.clone()),
        };
        let note = self.failure.clone().or(note);
        let failed = failed || self.failure.is_some();
        let centre = Rc::new(Cell::new(gpui::Point::default()));
        let label = div()
            .text_size(FS_LABEL)
            .font_weight(FontWeight::SEMIBOLD)
            .line_height(gpui::relative(1.3))
            .line_clamp(2)
            .child(super::widgets::text(self.label.clone()));
        let key = self.key.clone();
        let mut tile = div()
            .id(SharedString::from(format!("tile-{}", self.key)))
            .debug_selector(move || format!("tile-{key}"))
            .role(Role::Button)
            .aria_label(self.label.clone())
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .min_h(TILE_HEIGHT)
            .border_1()
            .border_color(if failed { theme.danger } else { theme.border })
            .rounded(RADIUS_MD)
            .bg(theme.surface_raised)
            .text_color(theme.on_surface)
            .overflow_hidden();
        tile = match self.face {
            Face::Button {
                shown,
                picture,
                glyph: state_glyph,
            } => tile
                .gap(SP_2)
                .p(SP_4)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(SP_3)
                        .min_w_0()
                        .when_some(picture, |head, picture| {
                            head.child(
                                img(picture)
                                    .size(px(56.))
                                    .flex_none()
                                    .rounded(RADIUS_SM)
                                    .object_fit(ObjectFit::Contain)
                                    .grayscale(self.gray),
                            )
                        })
                        .when_some(state_glyph, |head, g| {
                            let color = if g == Glyph::Muted { theme.danger } else { theme.on_surface };
                            head.child(
                                div()
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .justify_center()
                                    .size(px(56.))
                                    .child(glyph(g, px(43.), color)),
                            )
                        })
                        .child(label.pr(SP_5)),
                )
                .child(
                    div()
                        .text_size(FS_STATE)
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(gpui::relative(1.25))
                        .line_clamp(2)
                        .child(super::widgets::text(shown.text)),
                ),
            Face::Game { cover, whole, .. } => {
                let height = self.width * (215. / 460.);
                tile.child(div().w_full().h(height).flex_none().when_some(cover, |frame, cover| {
                    frame.child(
                        img(cover)
                            .size_full()
                            .when(whole, |img| img.p(SP_3))
                            .object_fit(if whole { ObjectFit::Contain } else { ObjectFit::Cover })
                            .grayscale(self.gray)
                            .with_fallback(|| div().into_any_element()),
                    )
                }))
                .child(label.px(SP_4).pt(SP_3).pb(SP_2))
            }
        };
        tile = tile.when_some(note, |tile, note| {
            tile.child(
                div()
                    .when(game, |n| n.px(SP_4).pb(SP_3))
                    .text_size(FS_SM)
                    .line_height(gpui::relative(1.5))
                    .text_color(if failed { theme.danger } else { theme.muted })
                    .child(super::widgets::text(note)),
            )
        });
        // What the tests read of the tile's state.
        for (on, what) in [(self.pending, "busy"), (failed, "failed"), (self.disabled, "disabled")] {
            if on {
                let key = self.key.clone();
                tile = tile.child(div().absolute().debug_selector(move || format!("{what}-{key}")));
            }
        }
        if self.pending {
            tile = tile.child(
                div()
                    .absolute()
                    .top(SP_4)
                    .right(SP_4)
                    .flex()
                    .when(game, |busy| busy.p(SP_1).rounded_full().bg(theme.surface_raised))
                    .child(spinner(theme.accent)),
            );
        }
        if self.disabled || self.inert {
            return long_press(
                SharedString::from(format!("press-{}", self.key)),
                tile.when(self.disabled, |tile| tile.opacity(0.5).aria_description("押せません")),
                None,
                centre,
            );
        }
        let press = self.on_press.clone();
        let menu = self.on_long.clone();
        let menu_centre = centre.clone();
        tile = tile
            .focusable()
            .tab_stop(true)
            .cursor_pointer()
            .hover({ let c = theme.hover_1; move |s| s.bg(c) })
            .active({ let c = theme.hover_2; move |s| s.bg(c) })
            .focus_visible({ let c = theme.accent; move |s| s.border_color(c) })
            .on_click(move |_, window, cx| press(window, cx))
            .on_key_down(move |event, window, cx| {
                if let Some(menu) = &menu
                    && is_menu_key(&event.keystroke)
                {
                    cx.stop_propagation();
                    menu(menu_centre.get(), window, cx);
                }
            });
        if let Some(drag) = self.drag {
            tile = tile.on_drag(drag, |game, _offset, _window, cx| {
                let name = game.name.clone();
                cx.new(|_| DragPreview(name))
            });
        }
        long_press(
            SharedString::from(format!("press-{}", self.key)),
            tile,
            self.on_long,
            centre,
        )
    }
}

/// The width of each of the columns that fill `width` at least `min` wide, `gap` apart.
pub fn columns(width: Pixels, min: Pixels, gap: Pixels) -> (usize, Pixels) {
    let width = f32::from(width).max(0.);
    let (min, gap) = (f32::from(min), f32::from(gap));
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // A few columns at most.
    let count = (((width + gap) / (min + gap)).floor() as usize).max(1);
    #[allow(clippy::cast_precision_loss)]
    let column = (width - gap * (count - 1) as f32) / count as f32;
    (count, px(column.max(0.)))
}

#[cfg(test)]
mod tests {
    use gpui::px;

    use super::columns;

    #[test]
    fn the_grid_has_seven_columns_at_1920_and_five_at_1280() {
        // The panel's page: 16px on each side, columns at least 240px, 12px apart.
        assert_eq!(columns(px(1920. - 32.), px(240.), px(12.)).0, 7);
        assert_eq!(columns(px(1280. - 32.), px(240.), px(12.)).0, 5);
        assert_eq!(columns(px(100.), px(240.), px(12.)).0, 1, "never none");
        let (count, width) = columns(px(504.), px(240.), px(12.));
        assert_eq!((count, width), (2, px(246.)));
    }
}
