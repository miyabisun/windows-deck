//! The design tokens of `DESIGN.md`: Sumi (dark) by default, Kinari when Windows is set to
//! light, and the sizes that follow from a button's minimum size.

use gpui::{App, BoxShadow, Global, Hsla, Pixels, WindowAppearance, hsla, point, px, rgb, rgba};

/// A button's minimum size; the tabs are half of it.
pub const TILE_WIDTH: Pixels = px(240.);
pub const TILE_HEIGHT: Pixels = px(144.);
/// A row to choose from (menus, label lists): as tall as a tab, since a finger is wide and
/// lands roughly.
pub const ROW_HEIGHT: Pixels = px(72.);
/// The height of the search field, chips and dialog buttons.
pub const CONTROL_HEIGHT: Pixels = px(56.);

pub const SP_1: Pixels = px(4.);
pub const SP_2: Pixels = px(8.);
pub const SP_3: Pixels = px(12.);
pub const SP_4: Pixels = px(16.);
pub const SP_5: Pixels = px(24.);

pub const RADIUS_SM: Pixels = px(6.);
pub const RADIUS_MD: Pixels = px(8.);
pub const RADIUS_LG: Pixels = px(12.);

pub const FS_SM: Pixels = px(14.);
pub const FS_MD: Pixels = px(16.);
pub const FS_LABEL: Pixels = px(18.);
pub const FS_STATE: Pixels = px(24.);

/// Windows' Japanese UI font, for the `system-ui` of the design.
pub const FONT: &str = "Yu Gothic UI";
/// For license keys.
pub const MONO: &str = "Consolas";

/// Figures of the same width, so that a changing value does not jump (`tabular-nums`).
pub fn tabular() -> gpui::FontFeatures {
    gpui::FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub surface: Hsla,
    pub surface_raised: Hsla,
    pub on_surface: Hsla,
    pub muted: Hsla,
    pub border: Hsla,
    pub accent: Hsla,
    pub accent_subtle: Hsla,
    pub danger: Hsla,
    pub danger_subtle: Hsla,
    pub hover_1: Hsla,
    pub hover_2: Hsla,
}

impl Global for Theme {}

impl Theme {
    pub fn sumi() -> Self {
        Self {
            dark: true,
            surface: rgb(0x0019_1919).into(),
            surface_raised: rgb(0x0023_2323).into(),
            on_surface: rgb(0x00e6_e6e6).into(),
            muted: rgb(0x009a_9a9a).into(),
            border: rgb(0x0033_3333).into(),
            accent: rgb(0x004c_c3cb).into(),
            accent_subtle: rgba(0x4cc3_cb26).into(),
            danger: rgb(0x00ff_6b6b).into(),
            danger_subtle: rgb(0x003a_1a1a).into(),
            hover_1: rgb(0x002c_2c2c).into(),
            hover_2: rgb(0x0036_3636).into(),
        }
    }

    pub fn kinari() -> Self {
        Self {
            dark: false,
            surface: rgb(0x00fa_f6ef).into(),
            surface_raised: rgb(0x00ff_fdf8).into(),
            on_surface: rgb(0x003a_2f28).into(),
            muted: rgb(0x006f_6257).into(),
            border: rgb(0x00e3_d9c9).into(),
            accent: rgb(0x0000_707a).into(),
            accent_subtle: rgba(0x0070_7a1a).into(),
            danger: rgb(0x009c_2b1d).into(),
            danger_subtle: rgb(0x00f9_e9e4).into(),
            hover_1: rgba(0x0070_7a14).into(),
            hover_2: rgba(0x0070_7a29).into(),
        }
    }

    /// The theme Windows asks for. A debug build can be told one with
    /// `WINDOWS_DECK_APPEARANCE=light` or `dark`, to look at both.
    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        if cfg!(debug_assertions) {
            match std::env::var("WINDOWS_DECK_APPEARANCE").as_deref() {
                Ok("light") => return Self::kinari(),
                Ok("dark") => return Self::sumi(),
                _ => {}
            }
        }
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::kinari(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::sumi(),
        }
    }

    /// The same theme without colour, under the resting cover.
    #[must_use]
    pub fn grayscale(self) -> Self {
        let g = |c: Hsla| c.grayscale();
        Self {
            dark: self.dark,
            surface: g(self.surface),
            surface_raised: g(self.surface_raised),
            on_surface: g(self.on_surface),
            muted: g(self.muted),
            border: g(self.border),
            accent: g(self.accent),
            accent_subtle: g(self.accent_subtle),
            danger: g(self.danger),
            danger_subtle: g(self.danger_subtle),
            hover_1: g(self.hover_1),
            hover_2: g(self.hover_2),
        }
    }

    pub fn get(cx: &App) -> Self {
        cx.try_global::<Self>().copied().unwrap_or_else(Self::sumi)
    }
}

/// The shadow of menus, dialogs and toasts: `0 8px 32px rgba(0,0,0,.25)`.
pub fn shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(0., 0., 0., 0.25),
        offset: point(px(0.), px(8.)),
        blur_radius: px(32.),
        spread_radius: px(0.),
        inset: false,
    }]
}

/// What covers the panel under a dialog: black at 50%.
pub fn backdrop() -> Hsla {
    hsla(0., 0., 0., 0.5)
}
