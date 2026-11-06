//! The panel: the virtual desktops' tabs on top, the buttons below, and what opens over them.

use std::{collections::HashMap, rc::Rc};

use gpui::{
    Anchor, AnyView, App, AppContext as _, Context, DismissEvent, Entity, FocusHandle, Focusable,
    FontWeight, IntoElement, MouseButton, ParentElement, Pixels, Point, Render, RetainAllImageCache,
    Role, ScrollHandle, SharedString, Styled, Subscription, Window, anchored, deferred, div, hsla,
    point, prelude::*, px,
};

use super::{
    dialogs::{Host, MenuGame, ProgramChooser, RoundDialog, game_menu, open},
    icons::{glyph, spinner},
    library::LibraryModal,
    mixer::MixerModal,
    theme::{FONT, FS_LABEL, FS_MD, FS_SM, FS_STATE, RADIUS_LG, RADIUS_MD, RADIUS_SM, ROW_HEIGHT, SP_1, SP_2, SP_3, SP_4, SP_5, TILE_HEIGHT, TILE_WIDTH, Theme, shadow},
    tile::{Face, Tile, columns},
    widgets::{Menu, MenuItem, Notice, OnLong, outlined_button},
};
use crate::{
    link::{Link, LinkStatus, PinAgain, RestStep, Started, item_key},
    model::{
        display::{Glyph, color_menu, describe_state, state_icon},
        library::{encode, picture_url, pins_of},
        protocol::{Desktop, Monitor, State},
        tabs::buttons_for,
    },
};

/// The configured monitor is missing: where to write it, and the monitors to choose from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setup {
    pub path: String,
    pub candidates: Vec<Monitor>,
}

pub struct Deck {
    link: Entity<Link>,
    /// The configured monitor, kept lit while resting.
    monitor: Option<String>,
    config_error: Option<String>,
    setup: Option<Setup>,
    /// This window's handle, which windows-link pins to every virtual desktop.
    hwnd: Option<isize>,
    /// Read the configuration again and place the window anew.
    reload: Rc<dyn Fn(&mut Window, &mut App)>,
    focus: FocusHandle,
    wake_focus: FocusHandle,
    scroll: ScrollHandle,
    tab_scroll: ScrollHandle,
    tab_focus: HashMap<String, FocusHandle>,
    add_focus: FocusHandle,
    desktop_menu: bool,
    desktop_menu_focus: FocusHandle,
    images: Entity<RetainAllImageCache>,
    library: Option<(Entity<LibraryModal>, Subscription)>,
    mixer: Option<(Entity<MixerModal>, Subscription)>,
    menu: Option<Menu>,
    dialog: Option<(AnyView, FocusHandle, Subscription)>,
    notice: Notice,
    was_resting: bool,
    _subscriptions: Vec<Subscription>,
}

impl Focusable for Deck {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Host for Deck {
    fn open_dialog(&mut self, view: AnyView, focus: FocusHandle, subscription: Subscription, _window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.dialog = Some((view, focus, subscription));
        cx.notify();
    }

    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn tell(&mut self, text: String, cx: &mut Context<Self>) {
        self.notice.show(text, cx, |this: &mut Self| &mut this.notice);
    }
}

impl Deck {
    pub fn new(
        link: Entity<Link>,
        monitor: Option<String>,
        config_error: Option<String>,
        setup: Option<Setup>,
        hwnd: Option<isize>,
        reload: Rc<dyn Fn(&mut Window, &mut App)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let observed = cx.observe_in(&link, window, |this: &mut Self, link, window, cx| {
            let resting = link.read(cx).resting;
            if resting && !this.was_resting {
                // The only thing to do while resting, so it has the focus.
                window.focus(&this.wake_focus, cx);
            } else if !resting && this.was_resting {
                window.focus(&this.focus, cx);
            }
            this.was_resting = resting;
            cx.notify();
        });
        // Stay on every virtual desktop; see `PinAgain` for when to pin again. windows-link may
        // also have been resting before this connection.
        let pinned = cx.subscribe(&link, |this: &mut Self, _, _: &PinAgain, cx| this.pin(cx));
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            cx.set_global(Theme::for_appearance(window.appearance()));
            cx.notify();
        });
        let mut deck = Self {
            link,
            monitor,
            config_error,
            setup,
            hwnd,
            reload,
            focus: cx.focus_handle(),
            wake_focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            tab_scroll: ScrollHandle::new(),
            tab_focus: HashMap::new(),
            add_focus: cx.focus_handle().tab_stop(true),
            desktop_menu: false,
            desktop_menu_focus: cx.focus_handle(),
            images: RetainAllImageCache::new(cx),
            library: None,
            mixer: None,
            menu: None,
            dialog: None,
            notice: Notice::default(),
            was_resting: false,
            _subscriptions: vec![observed, pinned, appearance],
        };
        if deck.link.read(cx).pin_round > 0 {
            deck.pin(cx);
        }
        window.focus(&deck.focus, cx);
        deck
    }

    pub fn link(&self) -> Entity<Link> {
        self.link.clone()
    }

    /// The library list open, if any.
    pub fn library(&self) -> Option<Entity<LibraryModal>> {
        self.library.as_ref().map(|(modal, _)| modal.clone())
    }

    pub fn desktop_menu_open(&self) -> bool {
        self.desktop_menu
    }

    fn show_menu(&mut self, menu: Menu, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&menu.focus, cx);
        self.menu = Some(menu);
        cx.notify();
    }

    fn pin(&mut self, cx: &mut Context<Self>) {
        if let Some(hwnd) = self.hwnd {
            self.link.update(cx, |l, cx| l.pin(hwnd, cx)).detach();
        }
        self.link.update(cx, |l, cx| l.sync_rest(cx)).detach();
    }

    fn open_library(&mut self, button: &str, label: &str, window: &mut Window, cx: &mut Context<Self>) {
        // Opening the library is the next press, which clears a failed update.
        self.link.update(cx, |l, _| {
            l.failures.remove(button);
        });
        let (link, button, label) = (self.link.clone(), button.to_owned(), label.to_owned());
        let modal = cx.new(|cx| LibraryModal::new(link, button, label, window, cx));
        let closed = cx.subscribe_in(&modal, window, |this, _, _: &DismissEvent, window, cx| {
            this.library = None;
            window.focus(&this.focus, cx);
            cx.notify();
        });
        let focus = modal.read(cx).search_focus(cx);
        self.library = Some((modal, closed));
        window.focus(&focus, cx);
        cx.notify();
    }

    fn open_mixer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let link = self.link.clone();
        let modal = cx.new(|cx| MixerModal::new(link, cx));
        let closed = cx.subscribe_in(&modal, window, |this, _, _: &DismissEvent, window, cx| {
            this.mixer = None;
            window.focus(&this.focus, cx);
            cx.notify();
        });
        let focus = modal.read(cx).focus_handle(cx);
        self.mixer = Some((modal, closed));
        window.focus(&focus, cx);
        cx.notify();
    }

    /// A shop's update shows how its round goes; Steam's says what it started.
    fn update_library(&mut self, button: String, label: String, rounds: bool, window: &mut Window, cx: &mut Context<Self>) {
        if rounds {
            self.follow_round(button.clone(), label, window, cx);
        }
        let started = self.link.update(cx, |l, cx| l.update_library(&button, cx));
        cx.spawn(async move |this, cx| {
            // A failure stays on the button (Link keeps it), and shows in the round's dialog.
            if let Some(started) = started.await
                && !rounds
            {
                let _ = this.update(cx, |this, cx| this.tell(started, cx));
            }
        })
        .detach();
    }

    fn follow_round(&mut self, button: String, label: String, window: &mut Window, cx: &mut Context<Self>) {
        let link = self.link.clone();
        let round = cx.new(|cx| RoundDialog::new(link, button, label, cx));
        open(self, round, window, cx);
    }

    fn start_pin(&mut self, button: String, id: String, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let started = self.link.update(cx, |l, cx| l.start_item(&button, &id, cx));
        cx.spawn_in(window, async move |this, cx| {
            if started.await == Started::Choose {
                let _ = this.update_in(cx, |this, window, cx| {
                    let link = this.link.clone();
                    let chooser = cx.new(|cx| ProgramChooser::new(link, button, id, name, Some(Rc::new(|_, _| {})), cx));
                    open(this, chooser, window, cx);
                });
            }
        })
        .detach();
    }

    fn overlay_open(&self) -> bool {
        self.library.is_some() || self.mixer.is_some() || self.menu.is_some() || self.dialog.is_some() || self.desktop_menu
    }

    #[allow(clippy::too_many_lines)] // The tab bar, top to bottom.
    fn tabs(&mut self, theme: &Theme, inert: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let link = self.link.read(cx);
        let mut desktops: Vec<Desktop> = link.desktops.clone();
        desktops.sort_by_key(|d| d.index);
        let disabled = !link.connected();
        let switching = link.switching.clone();
        let failure = link.desktop_failure.clone().or_else(|| (!link.resting).then(|| link.rest_failure.clone()).flatten());
        // Only the current tab is in the Tab order; the arrow keys move between tabs and Enter
        // or Space switches.
        let focusable = if desktops.iter().any(|d| d.current) { None } else { desktops.first().map(|d| d.id.clone()) };
        for desktop in &desktops {
            self.tab_focus.entry(desktop.id.clone()).or_insert_with(|| cx.focus_handle());
        }
        let order: Vec<FocusHandle> = desktops.iter().map(|d| self.tab_focus[&d.id].clone()).collect();
        let tabs = desktops.iter().enumerate().map(|(index, desktop)| {
            let handle = self.tab_focus[&desktop.id].clone().tab_stop(!inert && (desktop.current || Some(&desktop.id) == focusable.as_ref()));
            let id = desktop.id.clone();
            let current = desktop.current;
            let order = order.clone();
            let hover = theme.hover_1;
            div()
                .id(SharedString::from(format!("desktop-{}", desktop.id)))
                .debug_selector(|| format!("desktop-{}", desktop.name))
                .role(Role::Tab)
                .aria_label(desktop.name.clone())
                .aria_selected(current)
                .track_focus(&handle)
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .gap(SP_2)
                .h(TILE_HEIGHT / 2.)
                .min_w(TILE_WIDTH / 2.)
                .px(SP_5)
                .text_size(FS_LABEL)
                .whitespace_nowrap()
                .when(current, |tab| {
                    tab.text_color(theme.on_surface)
                        .font_weight(FontWeight::SEMIBOLD)
                        .border_b(px(3.))
                        .border_color(theme.accent)
                })
                .when(!current, |tab| tab.text_color(theme.muted))
                .when(disabled, |tab| tab.opacity(0.5))
                .when(!current && !disabled && !inert, |tab| tab.cursor_pointer().hover(move |s| s.bg(hover)))
                .child(desktop.name.clone())
                .when(current, |tab| {
                    let name = desktop.name.clone();
                    tab.child(div().absolute().debug_selector(move || format!("selected-{name}")))
                })
                .when(switching.as_deref() == Some(desktop.id.as_str()), |tab| tab.child(spinner(theme.accent)))
                .when(!inert, |tab| {
                    tab.on_click(cx.listener(move |this, _, _, cx| {
                        if !disabled && !current {
                            this.link.update(cx, |l, cx| l.switch_desktop(&id, cx)).detach();
                        }
                    }))
                    .on_key_down(move |event, window, cx| {
                        let next = match event.keystroke.key.as_str() {
                            "right" => Some((index + 1) % order.len()),
                            "left" => Some((index + order.len() - 1) % order.len()),
                            "home" => Some(0),
                            "end" => Some(order.len() - 1),
                            _ => None,
                        };
                        if let Some(next) = next {
                            cx.stop_propagation();
                            window.focus(&order[next], cx);
                        }
                    })
                })
        });
        let action = |id: &'static str, label: &'static str, icon: Glyph| {
            let hover = theme.hover_1;
            div()
                .id(id)
                .debug_selector(|| id.into())
                .role(Role::Button)
                .aria_label(label)
                .focusable()
                .tab_stop(!inert)
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(TILE_HEIGHT / 2.)
                .when(disabled, |a| a.opacity(0.5))
                .when(!disabled && !inert, |a| a.cursor_pointer().hover(move |s| s.bg(hover)))
                .child(glyph(icon, px(24.), theme.on_surface))
        };
        let add = action("add-desktop", "デスクトップを作る", Glyph::Plus)
            .track_focus(&self.add_focus)
            .aria_expanded(self.desktop_menu)
            .when(!inert, |a| {
                a.on_click(cx.listener(move |this, _, window, cx| {
                    if !disabled {
                        this.desktop_menu = !this.desktop_menu;
                        if this.desktop_menu {
                            window.focus(&this.desktop_menu_focus, cx);
                        }
                        cx.notify();
                    }
                }))
            });
        let rest = action("rest", "休止", Glyph::Moon).when(!inert, |a| {
            a.on_click(cx.listener(move |this, _, _, cx| {
                if !disabled {
                    let monitor = this.monitor.clone();
                    this.link.update(cx, |l, cx| l.rest(monitor.as_deref(), cx)).detach();
                }
            }))
        });
        div()
            .id("tab-bar")
            .role(Role::TabList)
            .aria_label("仮想デスクトップ")
            .flex()
            .flex_none()
            .items_center()
            .gap(SP_3)
            .px(SP_4)
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.surface)
            .child(
                div()
                    .id("tabs")
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .gap(SP_1)
                    .overflow_x_scroll()
                    .track_scroll(&self.tab_scroll)
                    .children(tabs),
            )
            .when_some(failure, |bar, failure| {
                bar.child(
                    div()
                        .id("tab-failure")
                        .debug_selector(|| "tab-failure".into())
                        .role(Role::Alert)
                        .aria_label(failure.clone())
                        .flex_shrink(1.)
                        .min_w_0()
                        .text_size(FS_SM)
                        .text_color(theme.danger)
                        .child(super::widgets::text(failure)),
                )
            })
            .child(div().flex().flex_none().gap(SP_1).child(add).child(rest))
    }

    fn desktop_menu(&self, theme: &Theme, top: Pixels, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let unmatched = self.link.read(cx).unmatched.clone();
        let close = cx.listener(|this, _, window, cx| {
            this.desktop_menu = false;
            window.focus(&this.add_focus, cx);
            cx.notify();
        });
        let items: Vec<gpui::AnyElement> = if unmatched.is_empty() {
            vec![div()
                .px(SP_4)
                .py(SP_3)
                .text_size(FS_SM)
                .text_color(theme.muted)
                .child(super::widgets::text("作れるデスクトップはありません。windows-link の desktops フォルダに「名前.yaml」を置くと、ここに出ます。"))
                .into_any_element()]
        } else {
            unmatched
                .into_iter()
                .map(|name| {
                    let hover = theme.hover_1;
                    let created = name.clone();
                    div()
                        .id(SharedString::from(format!("create-{name}")))
                        .debug_selector(|| format!("create-{name}"))
                        .role(Role::MenuItem)
                        .aria_label(name.clone())
                        .focusable()
                        .tab_stop(true)
                        .flex()
                        .items_center()
                        .min_h(ROW_HEIGHT)
                        .px(SP_4)
                        .text_size(FS_LABEL)
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .child(name)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.desktop_menu = false;
                            this.link.update(cx, |l, cx| l.create_desktop(&created, cx)).detach();
                            cx.notify();
                        }))
                        .into_any_element()
                })
                .collect()
        };
        let escape = cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                this.desktop_menu = false;
                window.focus(&this.add_focus, cx);
                cx.notify();
            }
        });
        deferred(
            div()
                .id("desktop-menu-layer")
                .track_focus(&self.desktop_menu_focus)
                .absolute()
                .inset_0()
                .occlude()
                .on_mouse_down(MouseButton::Left, close)
                .on_key_down(escape)
                .child(
                    anchored()
                        .anchor(Anchor::TopRight)
                        .position(point(px(f32::MAX / 4.), top))
                        .snap_to_window_with_margin(SP_4)
                        .child(
                            div()
                                .id("desktop-menu")
                                .role(Role::Menu)
                                .aria_label("作れるデスクトップ")
                                .occlude()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .min_w(px(240.))
                                .max_w(px(480.))
                                .py(SP_2)
                                .flex()
                                .flex_col()
                                .border_1()
                                .border_color(theme.border)
                                .rounded(RADIUS_LG)
                                .bg(theme.surface_raised)
                                .text_color(theme.on_surface)
                                .shadow(shadow())
                                .children(items),
                        ),
                ),
        )
        .with_priority(2)
    }

    #[allow(clippy::too_many_lines)] // Every button, then every pinned game.
    fn grid(&mut self, theme: &Theme, inert: bool, width: Pixels, window: &Window, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let _ = window;
        let link = self.link.read(cx);
        let base = link.base().to_owned();
        let disabled = !link.connected();
        let visible: Vec<_> = buttons_for(&link.buttons, &link.desktops).into_iter().cloned().collect();
        let pins = pins_of(&visible);
        let entity = cx.entity().downgrade();
        let mut tiles = Vec::new();
        for button in visible {
            let id = button.id.clone();
            let on_press: Rc<dyn Fn(&mut Window, &mut App)> = {
                let (entity, button) = (entity.clone(), button.clone());
                Rc::new(move |window, cx| {
                    let _ = entity.update(cx, |this, cx| match &button.state {
                        State::Library(_) => this.open_library(&button.id, &button.label, window, cx),
                        State::Mixer { .. } => this.open_mixer(window, cx),
                        _ => this.link.update(cx, |l, cx| l.press(&button.id, cx)).detach(),
                    });
                })
            };
            let on_long: Option<OnLong> = match &button.state {
                State::Library(library) => {
                    let (entity, id, label, rounds) = (entity.clone(), button.id.clone(), button.label.clone(), library.rounds);
                    Some(Rc::new(move |at: Point<Pixels>, window: &mut Window, cx: &mut App| {
                        let (id, label) = (id.clone(), label.clone());
                        let _ = entity.update(cx, |this, cx| {
                            let target = cx.entity().downgrade();
                            let mut items = vec![{
                                let (target, id, label) = (target.clone(), id.clone(), label.clone());
                                MenuItem::new("ライブラリを更新", move |window, cx| {
                                    let _ = target.update(cx, |this, cx| this.update_library(id.clone(), label.clone(), rounds, window, cx));
                                })
                            }];
                            if rounds {
                                let (id, label) = (id.clone(), label.clone());
                                items.push(MenuItem::new("更新の状況", move |window, cx| {
                                    let _ = target.update(cx, |this, cx| this.follow_round(id.clone(), label.clone(), window, cx));
                                }));
                            }
                            this.show_menu(Menu::new(at, label, items, cx), window, cx);
                        });
                    }))
                }
                State::Highlight { .. } => {
                    let (entity, id, label, state) = (entity.clone(), button.id.clone(), button.label.clone(), button.state.clone());
                    Some(Rc::new(move |at: Point<Pixels>, window: &mut Window, cx: &mut App| {
                        let items = color_menu(&state)
                            .into_iter()
                            .map(|choice| {
                                let (entity, id) = (entity.clone(), id.clone());
                                MenuItem::new(choice.label, move |_, cx| {
                                    let _ = entity.update(cx, |this, cx| {
                                        this.link.update(cx, |l, cx| l.set_color(&id, choice.color, cx)).detach();
                                    });
                                })
                            })
                            .collect();
                        let menu = Menu::new(at, label.clone(), items, cx);
                        let _ = entity.update(cx, |this, cx| this.show_menu(menu, window, cx));
                    }))
                }
                _ => None,
            };
            tiles.push(
                Tile {
                    key: id.clone().into(),
                    label: button.label.clone().into(),
                    face: Face::Button {
                        shown: describe_state(&button.state),
                        picture: button.icon.then(|| SharedString::from(format!("{base}/buttons/{}/icon", encode(&id)))),
                        glyph: if button.icon { None } else { state_icon(&button.state) },
                    },
                    pending: link.pending.contains(&id),
                    failure: link.failures.get(&id).cloned(),
                    disabled,
                    inert,
                    on_press,
                    on_long,
                    drag: None,
                    gray: link.resting,
                    width,
                }
                .render(theme)
                .into_any_element(),
            );
        }
        for pinned in pins {
            let key = item_key(&pinned.button, &pinned.pin.id);
            let on_press: Rc<dyn Fn(&mut Window, &mut App)> = {
                let (entity, pinned) = (entity.clone(), pinned.clone());
                Rc::new(move |window, cx| {
                    let _ = entity.update(cx, |this, cx| this.start_pin(pinned.button.clone(), pinned.pin.id.clone(), pinned.pin.name.clone(), window, cx));
                })
            };
            let on_long: OnLong = {
                let (entity, pinned, link_entity) = (entity.clone(), pinned.clone(), self.link.clone());
                Rc::new(move |at, window, cx| {
                    let game = MenuGame { id: pinned.pin.id.clone(), name: pinned.pin.name.clone(), installed: None, choosable: false, page: None };
                    let menu = game_menu(entity.clone(), &link_entity, &pinned.button, game, true, at, cx);
                    let _ = entity.update(cx, |this, cx| this.show_menu(menu, window, cx));
                })
            };
            tiles.push(
                Tile {
                    key: key.clone().into(),
                    label: pinned.pin.name.clone().into(),
                    face: Face::Game { cover: Some(picture_url(&base, &pinned.button, &pinned.pin.id).into()), whole: pinned.whole, note: None },
                    pending: link.pending.contains(&key),
                    failure: link.failures.get(&key).cloned(),
                    disabled,
                    inert,
                    on_press,
                    on_long: Some(on_long),
                    drag: None,
                    gray: link.resting,
                    width,
                }
                .render(theme)
                .into_any_element(),
            );
        }
        tiles
    }

    fn banner(text: impl Into<SharedString>, theme: &Theme) -> gpui::Stateful<gpui::Div> {
        let text = text.into();
        div()
            .id(SharedString::from(format!("banner-{}", text.len())))
            .aria_label(text.clone())
            .p(SP_3)
            .rounded(RADIUS_SM)
            .bg(theme.danger_subtle)
            .text_color(theme.danger)
            .text_size(FS_SM)
            .line_height(gpui::relative(1.5))
            .child(super::widgets::text(text))
    }

    fn rest_cover(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let link = self.link.read(cx);
        let pending = link.rest_pending == Some(RestStep::Wake);
        let failure = link.rest_failure.clone();
        // Drawn for the dark cover in either theme: faint white on black, never a bright face.
        let white = |a: f32| hsla(0., 0., 1., a);
        div()
            .id("rest")
            .role(Role::Dialog)
            .aria_label("休止中")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(SP_4)
            .p(SP_5)
            .bg(hsla(0., 0., 0., 0.7))
            .child(
                div()
                    .id("wake")
                    .debug_selector(|| "wake".into())
                    .role(Role::Button)
                    .aria_label("復帰")
                    .track_focus(&self.wake_focus)
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(SP_3)
                    .min_w(TILE_WIDTH)
                    .min_h(TILE_HEIGHT)
                    .px(SP_5)
                    .border_1()
                    .border_color(white(0.3))
                    .rounded(RADIUS_MD)
                    .bg(hsla(0., 0., 0., 0.6))
                    .text_color(white(0.8))
                    .text_size(FS_STATE)
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .when(pending, |w| w.child(spinner(white(0.8))))
                    .when(!pending, |w| w.child(glyph(Glyph::Sun, px(29.), white(0.8))))
                    .child("復帰")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !pending {
                            this.link.update(cx, |l, cx| l.wake(cx)).detach();
                        }
                    })),
            )
            .when_some(failure, |cover, failure| {
                cover.child(
                    div()
                        .id("rest-failure")
                        .role(Role::Status)
                        .aria_label(failure.clone())
                        .max_w(px(640.))
                        .p(SP_3)
                        .border_1()
                        .border_color(white(0.3))
                        .rounded(RADIUS_SM)
                        .text_color(white(0.8))
                        .text_size(FS_SM)
                        .child(super::widgets::text(failure)),
                )
            })
    }
}

impl Render for Deck {
    #[allow(clippy::too_many_lines)] // The page, top to bottom.
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let resting = self.link.read(cx).resting;
        let base_theme = Theme::get(cx);
        let theme = if resting { base_theme.grayscale() } else { base_theme };
        let inert = resting || self.overlay_open();
        let viewport = window.viewport_size();
        let (count, width) = columns(viewport.width - SP_4 * 2., TILE_WIDTH, SP_3);
        let tabs = self.tabs(&theme, inert, cx);
        let link = self.link.read(cx);
        let status = link.status;
        let base = link.base().to_owned();
        let has_buttons = !link.buttons.is_empty();
        let visible_count = buttons_for(&link.buttons, &link.desktops).len();
        let mut page = div()
            .id("deck")
            .flex()
            .flex_col()
            .gap(SP_3)
            .p(SP_4)
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll);
        if let Some(error) = self.config_error.clone() {
            page = page.child(Self::banner(format!("設定ファイルを読めないため、既定の設定で動いています: {error}"), &theme).role(Role::Alert));
        }
        if let Some(setup) = self.setup.clone() {
            let reload = self.reload.clone();
            page = page.child(
                div()
                    .id("setup")
                    .role(Role::Alert)
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(SP_2)
                    .p(SP_3)
                    .rounded(RADIUS_SM)
                    .bg(theme.danger_subtle)
                    .text_color(theme.danger)
                    .text_size(FS_SM)
                    .child(super::widgets::text(format!(
                        "表示するモニターが設定されていません。{} に monitor: モニターのID を書いて、再読み込みしてください。",
                        if setup.path.is_empty() { "設定ファイル" } else { setup.path.as_str() }
                    )))
                    .children(setup.candidates.iter().map(|m| {
                        div().pl(SP_5).child(format!(
                            "・{} {}{}{}",
                            m.id,
                            m.name.trim(),
                            if m.primary { "（メイン）" } else { "" },
                            if m.touch { "（タッチ）" } else { "" }
                        ))
                    }))
                    .child(
                        outlined_button("reload", "再読み込み", theme.danger, &theme)
                            .on_click(move |_, window, cx| reload(window, cx)),
                    ),
            );
        }
        if status == LinkStatus::Disconnected {
            page = page.child(Self::banner(format!("windows-link（{base}）に接続できません。再接続しています…"), &theme).role(Role::Status));
        }
        let quiet = |text: &str| super::widgets::text(text.to_owned()).text_size(FS_MD).text_color(theme.muted);
        page = if status == LinkStatus::Connecting && !has_buttons {
            page.child(
                div()
                    .id("connecting")
                    .role(Role::Status)
                    .flex()
                    .items_center()
                    .gap(SP_2)
                    .text_color(theme.muted)
                    .child(spinner(theme.accent))
                    .child("windows-link に接続しています…"),
            )
        } else if status == LinkStatus::Connected && !has_buttons {
            page.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(SP_2)
                    .child(quiet("ボタンがありません。"))
                    .child(quiet("windows-link の設定ファイル（%LOCALAPPDATA%\\windows-link\\config.yaml）にボタンを追加し、windows-link を再起動してください。")),
            )
        } else if visible_count == 0 {
            page.child(quiet("このデスクトップに割り当てたボタンはありません。"))
        } else {
            let tiles = self.grid(&theme, inert, width, window, cx);
            page.child(div().grid().grid_cols(u16::try_from(count).unwrap_or(1)).gap(SP_3).children(tiles))
        };
        let bar_bottom = TILE_HEIGHT / 2. + px(1.);
        let desktop_menu = self.desktop_menu.then(|| self.desktop_menu(&theme, bar_bottom, cx));
        let menu = self.menu.as_ref().map(|menu| {
            let this = cx.entity().downgrade();
            menu.render(&theme, Rc::new(move |window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.menu = None;
                    window.focus(&this.focus, cx);
                    cx.notify();
                });
            }))
        });
        div()
            .image_cache(self.images.clone())
            .id("root")
            .key_context("Deck")
            .track_focus(&self.focus)
            .on_action(|_: &super::FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &super::FocusPrev, window, cx| window.focus_prev(cx))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .font_family(FONT)
            .text_size(FS_MD)
            .bg(theme.surface)
            .text_color(theme.on_surface)
            .child(tabs)
            .child(page)
            .child(self.notice.render(&theme))
            .children(desktop_menu)
            .children(self.library.as_ref().map(|(modal, _)| modal.clone()))
            .children(self.mixer.as_ref().map(|(modal, _)| modal.clone()))
            .children(menu)
            .children(self.dialog.as_ref().map(|(view, _, _)| view.clone()))
            .when(resting, |root| root.child(self.rest_cover(cx)))
    }
}
