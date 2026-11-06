//! The parts every screen shares: toasts, long-press menus, the long press itself, dialogs'
//! frames and buttons.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use gpui::{
    AnyElement, App, Bounds, Context, DispatchPhase, Div, Element, ElementId, FontWeight,
    GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId,
    LongPressEvent, MouseButton, ParentElement, Pixels, Point, Role, SharedString, Stateful,
    Styled, Task, TouchPhase, Window, anchored, deferred, div, point, prelude::*, px,
};

use super::{
    icons::glyph,
    theme::{
        CONTROL_HEIGHT, FS_LABEL, FS_MD, FS_SM, RADIUS_LG, RADIUS_MD, ROW_HEIGHT, SP_2, SP_3, SP_4,
        SP_5, Theme, backdrop, shadow,
    },
};
use crate::model::display::Glyph;

/// What a button, a menu item or a cover does when pressed.
pub type Act = Rc<dyn Fn(&mut Window, &mut App)>;

/// What a name or a choice leads to.
pub type OnText = Rc<dyn Fn(String, &mut Window, &mut App)>;

/// What a game's start leads to, with its name.
pub type OnStarted = Rc<dyn Fn(String, &mut App)>;

/// A piece of text that the panel's tests can find (`text:<the text>`).
pub fn text(text: impl Into<SharedString>) -> Div {
    let text = text.into();
    let selector = text.clone();
    div()
        .debug_selector(move || format!("text:{selector}"))
        .child(text)
}

/// What a toast says, for a few seconds after each `show`.
#[derive(Default)]
pub struct Notice {
    pub text: Option<SharedString>,
    timer: Option<Task<()>>,
}

const TOAST_FOR: Duration = Duration::from_secs(3);

impl Notice {
    pub fn show<V: 'static>(
        &mut self,
        text: impl Into<SharedString>,
        cx: &mut Context<V>,
        notice: fn(&mut V) -> &mut Notice,
    ) {
        self.text = Some(text.into());
        self.timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_FOR).await;
            let _ = this.update(cx, |view, cx| {
                notice(view).text = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// The toast at the bottom centre of its container. It cannot be pressed and does not
    /// get in the way of what is under it.
    pub fn render(&self, theme: &Theme) -> impl IntoElement + use<> {
        div()
            .absolute()
            .bottom(SP_5)
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .children(self.text.clone().map(|text| {
                div()
                    .id("toast")
                    .role(Role::Status)
                    .aria_label(text.clone())
                    .max_w(px(960.))
                    .px(SP_4)
                    .py(SP_3)
                    .border_1()
                    .border_color(theme.border)
                    .rounded(RADIUS_MD)
                    .bg(theme.surface_raised)
                    .shadow(shadow())
                    .text_size(FS_MD)
                    .child(self::text(text))
            }))
    }
}

/// A choice in a long-press menu.
pub struct MenuItem {
    pub label: SharedString,
    pub disabled: bool,
    /// For a choice of one: whether it is the chosen one.
    pub checked: Option<bool>,
    pub action: Act,
}

impl MenuItem {
    pub fn new(
        label: impl Into<SharedString>,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            disabled: false,
            checked: None,
            action: Rc::new(action),
        }
    }

    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }
}

/// A menu that opens where the finger was, kept inside the window.
pub struct Menu {
    pub at: Point<Pixels>,
    pub title: SharedString,
    pub items: Vec<MenuItem>,
    /// Held while the menu is open, so that Esc and Tab stay in it.
    pub focus: gpui::FocusHandle,
}

impl Menu {
    pub fn new(
        at: Point<Pixels>,
        title: impl Into<SharedString>,
        items: Vec<MenuItem>,
        cx: &App,
    ) -> Self {
        Self {
            at,
            title: title.into(),
            items,
            focus: cx.focus_handle(),
        }
    }

    /// The menu over everything, closing (through `close`) on a choice, outside or Esc. The
    /// choice runs after the menu closed, so that it can open what comes next.
    #[allow(clippy::too_many_lines)] // One menu, top to bottom.
    pub fn render(&self, theme: &Theme, close: &Act) -> impl IntoElement + use<> {
        let items = self.items.iter().enumerate().map(|(index, item)| {
            let action = item.action.clone();
            let close = close.clone();
            let label = item.label.clone();
            let row = div()
                .id(("menu-item", index))
                .debug_selector(move || format!("menu-{label}"))
                .role(if item.checked.is_some() {
                    Role::RadioButton
                } else {
                    Role::MenuItem
                })
                .aria_label(item.label.clone())
                .flex()
                .items_center()
                .gap(SP_2)
                .min_h(ROW_HEIGHT)
                .px(SP_4)
                .text_size(FS_LABEL)
                .when_some(item.checked, |row, checked| {
                    row.aria_toggled(checked.into())
                        .when(checked, |row| {
                            let label = item.label.clone();
                            row.font_weight(FontWeight::SEMIBOLD).child(
                                div()
                                    .absolute()
                                    .debug_selector(move || format!("checked-{label}")),
                            )
                        })
                        .child(div().w(px(24.)).flex_none().when(checked, |mark| {
                            mark.child(glyph(Glyph::Check, px(24.), theme.accent))
                        }))
                })
                .child(item.label.clone());
            if item.disabled {
                let label = item.label.clone();
                row.opacity(0.5)
                    .child(
                        div()
                            .absolute()
                            .debug_selector(move || format!("disabled-{label}")),
                    )
                    .into_any_element()
            } else {
                row.focusable()
                    .tab_stop(true)
                    .focus_visible({
                        let c = theme.hover_2;
                        move |s| s.bg(c)
                    })
                    .cursor_pointer()
                    .hover({
                        let c = theme.hover_1;
                        move |s| s.bg(c)
                    })
                    .on_click(move |_, window, cx| {
                        close(window, cx);
                        action(window, cx);
                    })
                    .into_any_element()
            }
        });
        let outside = close.clone();
        deferred(
            div()
                .id("menu-layer")
                .track_focus(&self.focus)
                .absolute()
                .inset_0()
                .occlude()
                .on_mouse_down(MouseButton::Left, move |_, window, cx| outside(window, cx))
                .on_key_down({
                    let close = close.clone();
                    move |event, window, cx| {
                        if event.keystroke.key == "escape" {
                            close(window, cx);
                            cx.stop_propagation();
                        }
                    }
                })
                .child(
                    anchored()
                        .position(self.at)
                        .snap_to_window_with_margin(px(8.))
                        .child(
                            div()
                                .id("menu")
                                .role(Role::Menu)
                                .aria_label(self.title.clone())
                                .occlude()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .w(px(320.))
                                .py(SP_2)
                                .flex()
                                .flex_col()
                                .border_1()
                                .border_color(theme.border)
                                .rounded(RADIUS_LG)
                                .bg(theme.surface_raised)
                                .text_color(theme.on_surface)
                                .shadow(shadow())
                                .child(
                                    div()
                                        .px(SP_4)
                                        .py(SP_2)
                                        .text_size(FS_SM)
                                        .text_color(theme.muted)
                                        .truncate()
                                        .child(self.title.clone()),
                                )
                                .children(items),
                        ),
                ),
        )
        .with_priority(2)
    }
}

/// A dialog in the middle of the window over a black 50% cover; a tap on the cover or Esc
/// asks `close`.
pub fn modal<I: Into<ElementId>, L: Into<SharedString>>(
    id: I,
    label: L,
    theme: &Theme,
    close: Act,
    panel: Stateful<Div>,
) -> impl IntoElement + use<I, L> {
    let outside = close.clone();
    deferred(
        div()
            .id(id)
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(backdrop())
            .on_mouse_down(MouseButton::Left, move |_, window, cx| outside(window, cx))
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" {
                    close(window, cx);
                    cx.stop_propagation();
                }
            })
            .child(
                panel
                    .role(Role::Dialog)
                    .aria_label(label.into())
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .border_1()
                    .border_color(theme.border)
                    .rounded(RADIUS_LG)
                    .text_color(theme.on_surface)
                    .shadow(shadow()),
            ),
    )
    .with_priority(1)
}

/// The 56px × button that closes a dialog.
pub fn close_button(theme: &Theme, close: Act) -> Stateful<Div> {
    div()
        .id("close")
        .debug_selector(|| "close".into())
        .role(Role::Button)
        .aria_label("閉じる")
        .focusable()
        .tab_stop(true)
        .focus_visible({
            let c = theme.hover_2;
            move |s| s.bg(c)
        })
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(CONTROL_HEIGHT)
        .rounded(RADIUS_MD)
        .cursor_pointer()
        .hover({
            let c = theme.hover_1;
            move |s| s.bg(c)
        })
        .child(glyph(Glyph::Close, px(28.), theme.on_surface))
        .on_click(move |_, window, cx| close(window, cx))
}

/// A dialog's heading with its close button.
pub fn dialog_head<T: Into<SharedString>>(
    title: T,
    theme: &Theme,
    close: Act,
) -> impl IntoElement + use<T> {
    div()
        .flex()
        .items_center()
        .gap(SP_3)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(FS_LABEL)
                .font_weight(FontWeight::SEMIBOLD)
                .truncate()
                .child(text(title.into())),
        )
        .child(close_button(theme, close))
}

/// A 56px-high button with an outline (the accent, or the danger colour).
pub fn outlined_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    color: gpui::Hsla,
    theme: &Theme,
) -> Stateful<Div> {
    let label = label.into();
    let selector = label.clone();
    div()
        .id(id)
        .debug_selector(move || format!("button-{selector}"))
        .role(Role::Button)
        .aria_label(label.clone())
        .focusable()
        .tab_stop(true)
        .focus_visible({
            let c = theme.hover_2;
            move |s| s.bg(c)
        })
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(SP_2)
        .h(CONTROL_HEIGHT)
        .px(SP_5)
        .border_1()
        .border_color(color)
        .rounded(RADIUS_MD)
        .text_color(color)
        .text_size(FS_LABEL)
        .cursor_pointer()
        .hover({
            let c = theme.hover_1;
            move |s| s.bg(c)
        })
        .child(label)
}

/// What a long press opens, at the given point.
pub type OnLong = Rc<dyn Fn(Point<Pixels>, &mut Window, &mut App)>;

/// How long a finger or the mouse button must stay down.
const LONG_PRESS: Duration = Duration::from_millis(500);
/// How far the mouse may move meanwhile, so that a drag is no long press.
const SLOP: f32 = 10.;

/// A left button press that may become a long press.
#[derive(Default)]
struct MouseHold {
    origin: Point<Pixels>,
    armed: bool,
    fired: bool,
    timer: Option<Task<()>>,
}

/// Wrap `child` so that a long press of a finger (or the right button) calls `on_long` with
/// where it was; the press that opens the menu does not also tap. `centre` receives the
/// middle of the child, for the menu key.
pub fn long_press(
    id: impl Into<ElementId>,
    child: impl IntoElement,
    on_long: Option<OnLong>,
    centre: Rc<Cell<Point<Pixels>>>,
) -> LongPress {
    LongPress {
        id: id.into(),
        child: child.into_any_element(),
        on_long,
        centre,
    }
}

pub struct LongPress {
    id: ElementId,
    child: AnyElement,
    on_long: Option<OnLong>,
    centre: Rc<Cell<Point<Pixels>>>,
}

/// Whether a key opens the menu (the menu key or Shift+F10).
pub fn is_menu_key(keystroke: &gpui::Keystroke) -> bool {
    keystroke.key == "menu" || (keystroke.key == "f10" && keystroke.modifiers.shift)
}

impl IntoElement for LongPress {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for LongPress {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.child.prepaint(window, cx);
        self.on_long
            .is_some()
            .then(|| window.insert_hitbox(bounds, HitboxBehavior::Normal))
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
        let (Some(hitbox), Some(on_long), Some(id)) = (hitbox.take(), self.on_long.clone(), id)
        else {
            return;
        };
        let touch = on_long.clone();
        let touch_hitbox = hitbox.clone();
        window.on_mouse_event(move |event: &LongPressEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble
                || event.phase != TouchPhase::Started
                || window.default_prevented()
                || !touch_hitbox.is_hovered(window)
            {
                return;
            }
            window.prevent_default();
            cx.stop_propagation();
            touch(event.start_position, window, cx);
        });
        // The mouse's left button held still opens the menu too; the release that follows is
        // no click. The press outlives frames, so it is kept with the element.
        let held = window.with_element_state(id, |state: Option<Rc<RefCell<MouseHold>>>, _| {
            let state = state.unwrap_or_default();
            (state.clone(), state)
        });
        let (down, down_hitbox, mouse) = (held.clone(), hitbox.clone(), on_long.clone());
        window.on_mouse_event(move |event: &gpui::MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || !down_hitbox.is_hovered(window) {
                return;
            }
            if event.button == MouseButton::Right {
                cx.stop_propagation();
                mouse(event.position, window, cx);
                return;
            }
            if event.button != MouseButton::Left {
                return;
            }
            let (state, open, at) = (down.clone(), mouse.clone(), event.position);
            let timer = window.spawn(cx, async move |cx| {
                cx.background_executor().timer(LONG_PRESS).await;
                let fire = state.borrow().armed;
                if fire {
                    {
                        let mut state = state.borrow_mut();
                        state.armed = false;
                        state.fired = true;
                    }
                    let _ = cx.update(|window, cx| open(at, window, cx));
                }
            });
            *down.borrow_mut() = MouseHold {
                origin: at,
                armed: true,
                fired: false,
                timer: Some(timer),
            };
        });
        let moved = held.clone();
        window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, _, _| {
            let mut state = moved.borrow_mut();
            if phase == DispatchPhase::Bubble && state.armed {
                let (dx, dy) = (
                    event.position.x - state.origin.x,
                    event.position.y - state.origin.y,
                );
                if f32::from(dx).hypot(f32::from(dy)) > SLOP {
                    state.armed = false;
                    state.timer = None;
                }
            }
        });
        window.on_mouse_event(move |event: &gpui::MouseUpEvent, phase, _, cx| {
            if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                return;
            }
            let mut state = held.borrow_mut();
            state.armed = false;
            state.timer = None;
            if std::mem::take(&mut state.fired) {
                // The press opened the menu; it does not also press.
                cx.stop_propagation();
            }
        });
        self.centre.set(point(
            bounds.origin.x + bounds.size.width / 2.,
            bounds.origin.y + bounds.size.height / 2.,
        ));
    }
}
