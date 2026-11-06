//! The mixer: the whole volume on top, then each application with sound; tap where a slider
//! should be or slide it, and the speaker of a row mutes it.

use std::{cell::Cell, collections::HashMap, rc::Rc};

use gpui::{
    App, BorderStyle, Bounds, Context, DismissEvent, DispatchPhase, Element, ElementId, Entity,
    EventEmitter, FocusHandle, Focusable, FontWeight, GlobalElementId, Hitbox, HitboxBehavior,
    Hsla, InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, Render, Role, SharedString, Style, Styled, Task,
    TouchDragEvent, TouchPhase, Window, div, point, prelude::*, px, quad, relative, size,
};

use super::{
    icons::{glyph, spinner},
    theme::{FS_LABEL, FS_SM, MONO, ROW_HEIGHT, SP_2, SP_4, Theme},
    widgets::{dialog_head, modal},
};
use crate::{
    link::{Link, VolumeChange},
    model::{display::Glyph, protocol::Mixer},
};

/// One request at a time per slider: a value set meanwhile waits its turn, so a slide sends as
/// fast as windows-link answers and ends where the finger stopped.
#[derive(Default)]
struct Queue {
    busy: bool,
    next: Option<f64>,
}

pub struct MixerModal {
    link: Entity<Link>,
    mixer: Option<Mixer>,
    failure: Option<String>,
    queues: HashMap<String, Queue>,
    /// Whether a slider is held, by row.
    held: HashMap<String, Rc<Cell<bool>>>,
    focus: FocusHandle,
    _load: Task<()>,
}

impl EventEmitter<DismissEvent> for MixerModal {}

impl Focusable for MixerModal {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// A volume (0-1) as the percentage shown.
fn percent(volume: f64) -> i64 {
    #[allow(clippy::cast_possible_truncation)] // 0-100.
    let rounded = (volume * 100.).round() as i64;
    rounded
}

impl MixerModal {
    pub fn new(link: Entity<Link>, cx: &mut Context<Self>) -> Self {
        let read = link.update(cx, |l, cx| l.mixer(cx));
        let load = cx.spawn(async move |this, cx| {
            let read = read.await;
            let _ = this.update(cx, |this, cx| {
                match read {
                    Ok(mixer) => this.mixer = Some(mixer),
                    Err(failure) => this.failure = Some(failure),
                }
                cx.notify();
            });
        });
        Self {
            link,
            mixer: None,
            failure: None,
            queues: HashMap::new(),
            held: HashMap::new(),
            focus: cx.focus_handle(),
            _load: load,
        }
    }

    /// Send `volume` for the row `key`, after the request under way.
    fn change(&mut self, key: String, volume: f64, cx: &mut Context<Self>) {
        let queue = self.queues.entry(key.clone()).or_default();
        queue.next = Some(volume);
        if queue.busy {
            return;
        }
        queue.busy = true;
        let link = self.link.clone();
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(Some(sending)) = this.update(cx, |this, _| {
                    this.queues.get_mut(&key).and_then(|q| q.next.take())
                }) else {
                    break;
                };
                let change = VolumeChange { volume: Some(sending), muted: None };
                let sent = link.update(cx, |l, cx| match key.strip_prefix("app:") {
                    Some(process) => l.set_app(process, change, cx),
                    None => l.set_master(change, cx),
                });
                let result = sent.await;
                let _ = this.update(cx, |this, cx| {
                    let waiting = this.queues.get(&key).is_some_and(|q| q.next.is_some());
                    match result {
                        // Take windows-link's answer only once the slider has stopped.
                        Ok(mixer) if !waiting => {
                            this.mixer = Some(mixer);
                            this.failure = None;
                        }
                        Ok(_) => this.failure = None,
                        Err(failure) => this.failure = Some(failure),
                    }
                    cx.notify();
                });
            }
            let _ = this.update(cx, |this, _| {
                if let Some(queue) = this.queues.get_mut(&key) {
                    queue.busy = false;
                }
            });
        })
        .detach();
    }

    fn set_whole(&mut self, volume: f64, cx: &mut Context<Self>) {
        let Some(mixer) = &mut self.mixer else { return };
        // Moving the volume unmutes, as Windows' own slider does.
        mixer.master.volume = volume;
        mixer.master.muted = false;
        cx.notify();
        self.change("master".into(), volume, cx);
    }

    fn set_app(&mut self, process: &str, volume: f64, cx: &mut Context<Self>) {
        let Some(app) = self.mixer.as_mut().and_then(|m| m.apps.iter_mut().find(|a| a.process == process)) else {
            return;
        };
        app.volume = volume;
        app.muted = false;
        cx.notify();
        self.change(format!("app:{process}"), volume, cx);
    }

    /// Mute or unmute, as the speaker of a row in Windows' volume mixer does, and take
    /// windows-link's answer unless a slider is still moving.
    fn toggle(&mut self, process: Option<String>, cx: &mut Context<Self>) {
        let Some(mixer) = &mut self.mixer else { return };
        let muted = match &process {
            None => {
                mixer.master.muted = !mixer.master.muted;
                mixer.master.muted
            }
            Some(process) => {
                let Some(app) = mixer.apps.iter_mut().find(|a| &a.process == process) else { return };
                app.muted = !app.muted;
                app.muted
            }
        };
        cx.notify();
        let change = VolumeChange { volume: None, muted: Some(muted) };
        let sent = self.link.update(cx, |l, cx| match &process {
            Some(process) => l.set_app(process, change, cx),
            None => l.set_master(change, cx),
        });
        cx.spawn(async move |this, cx| {
            let result = sent.await;
            let _ = this.update(cx, |this, cx| {
                let sliding = this.queues.values().any(|q| q.busy);
                match result {
                    Ok(mixer) if !sliding => {
                        this.mixer = Some(mixer);
                        this.failure = None;
                    }
                    Ok(_) => this.failure = None,
                    Err(failure) => this.failure = Some(failure),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn row(
        &mut self,
        key: &str,
        name: SharedString,
        level: (f64, bool),
        whole: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let (volume, muted) = level;
        let process = key.strip_prefix("app:").map(str::to_owned);
        let held = self.held.entry(key.to_owned()).or_default().clone();
        let entity = cx.entity().downgrade();
        let set = {
            let (entity, process) = (entity.clone(), process.clone());
            Rc::new(move |value: f64, _: &mut Window, cx: &mut App| {
                let _ = entity.update(cx, |this, cx| match &process {
                    Some(process) => this.set_app(process, value, cx),
                    None => this.set_whole(value, cx),
                });
            })
        };
        let step = {
            let set = set.clone();
            move |event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut App| {
                let delta = match event.keystroke.key.as_str() {
                    "right" | "up" => 0.01,
                    "left" | "down" => -0.01,
                    _ => return,
                };
                cx.stop_propagation();
                set((volume + delta).clamp(0., 1.), window, cx);
            }
        };
        let toggle_process = process.clone();
        let speaker = div()
            .id(SharedString::from(format!("speaker-{key}")))
            .debug_selector(|| format!("speaker-{name}"))
            .role(Role::Button)
            .aria_label(format!("{name}のミュート"))
            .aria_toggled(muted.into())
            .focusable()
            .tab_stop(true)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(56.))
            .cursor_pointer()
            .child(glyph(
                if muted { Glyph::Muted } else { Glyph::Sound },
                px(28.),
                if muted { theme.danger } else { theme.on_surface },
            ))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle(toggle_process.clone(), cx)));
        div()
            .flex()
            .items_center()
            .gap(SP_4)
            .min_h(ROW_HEIGHT)
            .when(whole, |row| row.pb(SP_2).border_b_1().border_color(theme.border))
            .child(speaker)
            .child(
                div()
                    .w(px(200.))
                    .flex_none()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .text_size(FS_LABEL)
                    .when(whole, |n| n.font_weight(FontWeight::SEMIBOLD))
                    .child(div().truncate().child(name.clone()))
                    .when(muted, |n| {
                        let selector = name.clone();
                        n.child(
                            div()
                                .debug_selector(move || format!("muted-{selector}"))
                                .text_size(FS_SM)
                                .font_weight(FontWeight::NORMAL)
                                .text_color(theme.danger)
                                .child("ミュート中"),
                        )
                    }),
            )
            .child(
                div()
                    .id(SharedString::from(format!("slider-{key}")))
                    .debug_selector(|| format!("slider-{name}"))
                    .role(Role::Slider)
                    .aria_label(name.clone())
                    .aria_numeric_value(f64::from(u8::try_from(percent(volume)).unwrap_or(0)))
                    .aria_min_numeric_value(0.)
                    .aria_max_numeric_value(100.)
                    .focusable()
                    .tab_stop(true)
                    .flex_1()
                    .min_w_0()
                    .on_key_down(step)
                    .child(Slider {
                        id: SharedString::from(format!("track-{key}")).into(),
                        value: volume,
                        fill: theme.accent,
                        rest: theme.border,
                        ring: theme.surface_raised,
                        held,
                        on_change: set,
                    }),
            )
            .child(
                div()
                    .w(px(81.))
                    .flex_none()
                    .flex()
                    .justify_end()
                    .font_family(MONO)
                    .text_size(FS_LABEL)
                    .child({
                        let (selector, value) = (name.clone(), percent(volume));
                        div().debug_selector(move || format!("value-{selector}-{value}")).child(format!("{value}%"))
                    }),
            )
    }
}

impl Render for MixerModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let this = cx.entity().downgrade();
        let close: Rc<dyn Fn(&mut Window, &mut App)> = Rc::new(move |_, cx| {
            let _ = this.update(cx, |_, cx| cx.emit(DismissEvent));
        });
        let body = match self.mixer.clone() {
            Some(mixer) => {
                let whole = self.row("master", "全体".into(), (mixer.master.volume, mixer.master.muted), true, &theme, cx).into_any_element();
                let apps: Vec<_> = mixer
                    .apps
                    .iter()
                    .map(|app| {
                        self.row(&format!("app:{}", app.process), app.name.clone().into(), (app.volume, app.muted), false, &theme, cx)
                            .into_any_element()
                    })
                    .collect();
                div()
                    .flex()
                    .flex_col()
                    .gap(SP_2)
                    .child(whole)
                    .child(
                        div()
                            .id("apps")
                            .flex()
                            .flex_col()
                            .overflow_y_scroll()
                            .when(apps.is_empty(), |list| {
                                list.child(super::widgets::text("音を出しているアプリはありません").text_size(FS_SM).text_color(theme.muted))
                            })
                            .children(apps),
                    )
                    .into_any_element()
            }
            None if self.failure.is_none() => div()
                .flex()
                .items_center()
                .gap(SP_2)
                .text_size(FS_SM)
                .text_color(theme.muted)
                .child(spinner(theme.accent))
                .child("読み込んでいます…")
                .into_any_element(),
            None => div().into_any_element(),
        };
        let panel = div()
            .id("mixer")
            .track_focus(&self.focus)
            .w(px(960.))
            .max_w(relative(0.95))
            .max_h(relative(0.8))
            .p(SP_4)
            .flex()
            .flex_col()
            .gap(SP_2)
            .bg(theme.surface_raised)
            .child(dialog_head("ミキサー", &theme, close.clone()))
            .when_some(self.failure.clone(), |p, f| {
                p.child(super::widgets::text(f).text_size(FS_SM).text_color(theme.danger))
            })
            .child(body);
        modal("mixer-layer", "ミキサー", &theme, close, panel)
    }
}

/// A 12px groove filled with the accent up to the value, and a 36px knob; the whole row
/// height takes a tap or a slide, of the mouse or of a finger.
struct Slider {
    id: ElementId,
    value: f64,
    fill: Hsla,
    rest: Hsla,
    ring: Hsla,
    held: Rc<Cell<bool>>,
    on_change: Rc<dyn Fn(f64, &mut Window, &mut App)>,
}

const KNOB: Pixels = px(36.);

/// The value under `x` on a groove at `left` that is `width` wide; the knob's middle stays on
/// it, as with a browser's range input.
pub fn value_at(x: Pixels, left: Pixels, width: Pixels) -> f64 {
    let travel = f32::from(width - KNOB).max(1.);
    f64::from((f32::from(x - left - KNOB / 2.) / travel).clamp(0., 1.))
}

impl IntoElement for Slider {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Slider {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

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
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = ROW_HEIGHT.into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let groove = px(12.);
        let middle = bounds.origin.y + bounds.size.height / 2.;
        #[allow(clippy::cast_possible_truncation)] // 0-1.
        let value = self.value.clamp(0., 1.) as f32;
        let knob_x = bounds.origin.x + KNOB / 2. + (bounds.size.width - KNOB) * value;
        let track = Bounds::new(point(bounds.origin.x, middle - groove / 2.), size(bounds.size.width, groove));
        window.paint_quad(quad(track, px(6.), self.rest, px(0.), self.rest, BorderStyle::Solid));
        let filled = Bounds::new(track.origin, size(knob_x - bounds.origin.x, groove));
        window.paint_quad(quad(filled, px(6.), self.fill, px(0.), self.fill, BorderStyle::Solid));
        let knob = Bounds::new(point(knob_x - KNOB / 2., middle - KNOB / 2.), size(KNOB, KNOB));
        window.paint_quad(quad(knob, KNOB / 2., self.fill, px(3.), self.ring, BorderStyle::Solid));

        let (left, width) = (bounds.origin.x, bounds.size.width);
        let held = self.held.clone();
        let change = self.on_change.clone();
        let down_hitbox = hitbox.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble && event.button == MouseButton::Left && down_hitbox.is_hovered(window) {
                held.set(true);
                window.capture_pointer(down_hitbox.id);
                change(value_at(event.position.x, left, width), window, cx);
            }
        });
        let (held, change) = (self.held.clone(), self.on_change.clone());
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble && held.get() && event.pressed_button == Some(MouseButton::Left) {
                change(value_at(event.position.x, left, width), window, cx);
            }
        });
        let held = self.held.clone();
        window.on_mouse_event(move |_: &MouseUpEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble && held.replace(false) {
                window.release_pointer();
            }
        });
        let (held, change, touch_hitbox) = (self.held.clone(), self.on_change.clone(), hitbox.clone());
        window.on_mouse_event(move |event: &TouchDragEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            match event.phase {
                TouchPhase::Started if touch_hitbox.is_hovered(window) && !window.default_prevented() => {
                    // A finger on the groove slides it rather than scrolling the list.
                    window.prevent_default();
                    held.set(true);
                    change(value_at(event.position.x, left, width), window, cx);
                }
                TouchPhase::Moved if held.get() => change(value_at(event.position.x, left, width), window, cx),
                TouchPhase::Ended | TouchPhase::Cancelled if held.replace(false) => {
                    change(value_at(event.position.x, left, width), window, cx);
                }
                _ => {}
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use gpui::px;

    use super::value_at;

    #[test]
    fn a_tap_sets_the_value_under_the_knobs_middle() {
        // A 436px groove at 100: the knob's middle runs from 118 to 518.
        assert!((value_at(px(118.), px(100.), px(436.)) - 0.0).abs() < 1e-6, "left end");
        assert!((value_at(px(318.), px(100.), px(436.)) - 0.5).abs() < 1e-6, "middle");
        assert!((value_at(px(518.), px(100.), px(436.)) - 1.0).abs() < 1e-6, "right end");
        assert!((value_at(px(0.), px(100.), px(436.))).abs() < 1e-6, "clamped low");
        assert!((value_at(px(900.), px(100.), px(436.)) - 1.0).abs() < 1e-6, "clamped high");
    }
}
