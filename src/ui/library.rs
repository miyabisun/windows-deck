//! A library's games in a dialog covering 80% of the screen: search, labels, the order, and
//! a grid of games that only reads the pictures it shows.

use std::{collections::HashMap, rc::Rc};

use gpui::{
    AnyView, App, AppContext as _, Context, DismissEvent, Entity, EventEmitter, FocusHandle,
    Focusable, FontWeight, Global, IntoElement, ListAlignment, ListState, ParentElement, Pixels,
    Point, Render, RetainAllImageCache, Role, SharedString, Styled, Subscription, Task, Window,
    div, list, prelude::*, px, relative,
};

use super::{
    dialogs::{AskDialog, Host, MenuGame, ProgramChooser, game_menu, open},
    icons::{glyph, spinner},
    text_input::{Changed, TextInput},
    theme::{
        CONTROL_HEIGHT, FS_LABEL, FS_SM, RADIUS_MD, RADIUS_SM, SP_2, SP_3, SP_4, SP_5, TILE_WIDTH,
        Theme,
    },
    tile::{DraggedGame, Face, Tile, columns},
    widgets::{Act, Menu, MenuItem, Notice, OnStarted, close_button, long_press, modal},
};
use crate::{
    link::{Link, Started, item_key},
    model::{
        display::Glyph,
        library::{
            Filter, Order, UNLABELED, date_label, labels_locked_reason, orders_of, partial_reason,
            picture_url, shop_name, sort_items, toggle_label, visible_items,
        },
        protocol::{Item, Label, Library, State},
    },
};

/// Each library's last order, kept while the panel runs.
#[derive(Default)]
struct Orders(HashMap<String, Order>);

impl Global for Orders {}

pub struct LibraryModal {
    link: Entity<Link>,
    button: String,
    label: String,
    library: Option<Library>,
    failure: Option<String>,
    signing_in: bool,
    search: Entity<TextInput>,
    active: Vec<String>,
    notice: Notice,
    menu: Option<Menu>,
    dialog: Option<(AnyView, FocusHandle, Subscription)>,
    list: ListState,
    /// The games shown and the columns, as the list was last laid out.
    laid_out: (Vec<String>, usize),
    images: Entity<RetainAllImageCache>,
    focus: FocusHandle,
    _search: Subscription,
    _link: Subscription,
    loading: Option<Task<()>>,
}

impl EventEmitter<DismissEvent> for LibraryModal {}

impl Focusable for LibraryModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.read(cx).focus_handle(cx)
    }
}

impl Host for LibraryModal {
    fn open_dialog(
        &mut self,
        view: AnyView,
        focus: FocusHandle,
        subscription: Subscription,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        self.dialog = Some((view, focus, subscription));
        cx.notify();
    }

    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog = None;
        window.focus(&self.search.read(cx).focus_handle(cx), cx);
        cx.notify();
    }

    fn tell(&mut self, text: String, cx: &mut Context<Self>) {
        self.notice
            .show(text, cx, |this: &mut Self| &mut this.notice);
    }

    fn changed(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.load(cx);
    }
}

impl LibraryModal {
    pub fn new(
        link: Entity<Link>,
        button: String,
        label: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("名前で探す", "", cx));
        let search_changed = cx.subscribe_in(&search, window, |this, _, _: &Changed, _, cx| {
            this.list.reset(0);
            this.laid_out = (Vec::new(), 0);
            cx.notify();
        });
        let observed = cx.observe(&link, |_, _, cx| cx.notify());
        let mut modal = Self {
            link,
            button,
            label,
            library: None,
            failure: None,
            signing_in: false,
            search,
            active: vec![UNLABELED.into()],
            notice: Notice::default(),
            menu: None,
            dialog: None,
            list: ListState::new(0, ListAlignment::Top, px(600.)),
            laid_out: (Vec::new(), 0),
            images: RetainAllImageCache::new(cx),
            focus: cx.focus_handle(),
            _search: search_changed,
            _link: observed,
            loading: None,
        };
        modal.load(cx);
        modal
    }

    pub fn search_focus(&self, cx: &App) -> FocusHandle {
        self.search.read(cx).focus_handle(cx)
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let read = self.link.update(cx, |l, cx| l.library(&self.button, cx));
        self.loading = Some(cx.spawn(async move |this, cx| {
            let read = read.await;
            let _ = this.update(cx, |this, cx| {
                match read {
                    Ok(library) => {
                        // Forget selected labels that no longer exist.
                        this.active.retain(|id| {
                            id == UNLABELED || library.labels.iter().any(|l| &l.id == id)
                        });
                        this.library = Some(library);
                        this.failure = None;
                    }
                    Err(failure) => {
                        this.library = None;
                        this.failure = Some(failure);
                    }
                }
                cx.notify();
            });
        }));
    }

    fn state(&self, cx: &App) -> Option<crate::model::protocol::LibraryState> {
        self.link
            .read(cx)
            .buttons
            .iter()
            .find(|b| b.id == self.button)
            .and_then(|b| match &b.state {
                State::Library(library) => Some(library.clone()),
                _ => None,
            })
    }

    fn order(&self, cx: &App) -> Order {
        let chosen = cx
            .try_global::<Orders>()
            .and_then(|o| o.0.get(&self.button).copied())
            .unwrap_or(Order::Recent);
        let offered = self
            .library
            .as_ref()
            .map(|l| orders_of(&l.items))
            .unwrap_or_default();
        // An order whose dates are gone (the purchases cannot be read) falls back.
        if offered.contains(&chosen) {
            chosen
        } else {
            Order::Recent
        }
    }

    fn shown(&self, cx: &App) -> Vec<Item> {
        let Some(library) = &self.library else {
            return Vec::new();
        };
        let query = self.search.read(cx).text().to_owned();
        let sorted = sort_items(&library.items, self.order(cx));
        visible_items(
            sorted,
            Filter {
                query: &query,
                active: &self.active,
                hide: &library.hide,
            },
        )
        .into_iter()
        .cloned()
        .collect()
    }

    fn locked(&self) -> Option<String> {
        labels_locked_reason(
            self.library
                .as_ref()
                .and_then(|l| l.labels_locked.as_deref()),
        )
    }

    fn tell_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.notice
            .show(text, cx, |this: &mut Self| &mut this.notice);
    }

    /// Start a game; the list stays open where it was, to pick the next one or its label.
    fn start(&mut self, item: &Item, window: &mut Window, cx: &mut Context<Self>) {
        let (id, name) = (item.id.clone(), item.name.clone());
        let started = self
            .link
            .update(cx, |l, cx| l.start_item(&self.button, &id, cx));
        cx.spawn_in(window, async move |this, cx| {
            let result = started.await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Started::Started => this.tell_text(format!("「{name}」を起動しました"), cx),
                Started::Choose => this.choose_program(id, name, window, cx),
                Started::Failed => {}
            });
        })
        .detach();
    }

    fn choose_program(
        &mut self,
        id: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let this = cx.entity().downgrade();
        let started: OnStarted = Rc::new(move |name, cx| {
            let _ = this.update(cx, |this, cx| {
                this.tell_text(format!("「{name}」を起動しました"), cx);
            });
        });
        let (link, button) = (self.link.clone(), self.button.clone());
        let chooser = cx.new(|cx| ProgramChooser::new(link, button, id, name, Some(started), cx));
        open(self, chooser, window, cx);
    }

    fn sign_in(&mut self, cx: &mut Context<Self>) {
        let Some(shop) = self.library.as_ref().and_then(|l| l.sign_in.clone()) else {
            return;
        };
        self.signing_in = true;
        cx.notify();
        let done = self.link.update(cx, |l, cx| l.sign_in(&shop, cx));
        cx.spawn(async move |this, cx| {
            let failed = done.await;
            let _ = this.update(cx, |this, cx| {
                this.signing_in = false;
                match failed {
                    Some(failed) => this.tell_text(failed, cx),
                    None => this.load(cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Run a label change, then show its failure or read the library again.
    fn change_labels(change: Task<Option<String>>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let failed = change.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(failed) = failed {
                    this.tell_text(failed, cx);
                }
                this.load(cx);
            });
        })
        .detach();
    }

    fn drop_game(&mut self, game: &DraggedGame, label: &Label, cx: &mut Context<Self>) {
        let Some(item) = self
            .library
            .as_ref()
            .and_then(|l| l.items.iter().find(|i| i.id == game.id))
            .cloned()
        else {
            return;
        };
        if item.labels.contains(&label.id) {
            self.tell_text(
                format!("「{}」にはもう「{}」が付いています", item.name, label.name),
                cx,
            );
            return;
        }
        let put = self.link.update(cx, |l, cx| {
            l.set_label(&self.button, &label.id, &item.id, true, cx)
        });
        let (name, label_name) = (item.name.clone(), label.name.clone());
        cx.spawn(async move |this, cx| {
            let failed = put.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(failed) = failed {
                    this.tell_text(failed, cx);
                } else {
                    this.tell_text(format!("「{name}」に「{label_name}」を付けました"), cx);
                    this.load(cx);
                }
            });
        })
        .detach();
    }

    fn ask(&mut self, ask: Entity<AskDialog>, window: &mut Window, cx: &mut Context<Self>) {
        open(self, ask, window, cx);
    }

    fn ask_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let ask = cx.new(|cx| {
            AskDialog::new(
                "新しいラベル",
                None,
                Some(""),
                "作る",
                false,
                move |name, _, cx| {
                    let _ = this.update(cx, |this, cx| {
                        let made = this
                            .link
                            .update(cx, |l, cx| l.create_label(&this.button, &name, cx));
                        let change = cx.background_spawn(async move { made.await.err() });
                        Self::change_labels(change, cx);
                    });
                },
                cx,
            )
        });
        self.ask(ask, window, cx);
    }

    fn show_menu(&mut self, menu: Menu, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&menu.focus, cx);
        self.menu = Some(menu);
        cx.notify();
    }

    fn label_menu(
        &mut self,
        label: Label,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let fixed = !label.editable || self.locked().is_some();
        let this = cx.entity().downgrade();
        let rename = {
            let (this, label) = (this.clone(), label.clone());
            MenuItem::new("名前変更", move |window, cx| {
                let label = label.clone();
                let _ = this.update(cx, |host, cx| {
                    let target = cx.entity().downgrade();
                    let id = label.id.clone();
                    let ask = cx.new(|cx| {
                        AskDialog::new(
                            "ラベルの名前変更",
                            None,
                            Some(&label.name),
                            "変更する",
                            false,
                            move |name, _, cx| {
                                let _ = target.update(cx, |this, cx| {
                                    let change = this.link.update(cx, |l, cx| {
                                        l.rename_label(&this.button, &id, &name, cx)
                                    });
                                    Self::change_labels(change, cx);
                                });
                            },
                            cx,
                        )
                    });
                    host.ask(ask, window, cx);
                });
            })
            .disabled(fixed)
        };
        let delete = {
            let label = label.clone();
            MenuItem::new("削除", move |window, cx| {
                let label = label.clone();
                let _ = this.update(cx, |host, cx| {
                    let target = cx.entity().downgrade();
                    let id = label.id.clone();
                    let ask = cx.new(|cx| {
                        AskDialog::new(
                            format!("「{}」を削除しますか？", label.name),
                            Some("ラベルだけを消します。ゲームはライブラリに残ります。"),
                            None,
                            "削除する",
                            true,
                            move |_, _, cx| {
                                let _ = target.update(cx, |this, cx| {
                                    let change = this
                                        .link
                                        .update(cx, |l, cx| l.delete_label(&this.button, &id, cx));
                                    Self::change_labels(change, cx);
                                });
                            },
                            cx,
                        )
                    });
                    host.ask(ask, window, cx);
                });
            })
            .disabled(fixed)
        };
        let menu = Menu::new(at, label.name, vec![rename, delete], cx);
        self.show_menu(menu, window, cx);
    }

    fn order_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.order(cx);
        let offered = self
            .library
            .as_ref()
            .map(|l| orders_of(&l.items))
            .unwrap_or_default();
        let items = offered
            .into_iter()
            .map(|order| {
                let this = cx.entity().downgrade();
                MenuItem::new(order.label(), move |_, cx| {
                    let _ = this.update(cx, |this, cx| {
                        cx.default_global::<Orders>()
                            .0
                            .insert(this.button.clone(), order);
                        this.list.reset(0);
                        this.laid_out = (Vec::new(), 0);
                        cx.notify();
                    });
                })
                .checked(order == current)
            })
            .collect();
        let menu = Menu::new(at, "並び順", items, cx);
        self.show_menu(menu, window, cx);
    }

    fn note_of(item: &Item, order: Order, pinned: &[String]) -> Option<String> {
        let mut notes = Vec::new();
        // The date the list is in order of, first, so the order shows for itself.
        if let (Some(note), Some(date)) = (order.note(), order.date(item)) {
            notes.push(format!("{note} {}", date_label(date, &chrono::Local)));
        }
        if let Some(detail) = item.detail.as_ref().filter(|d| !d.is_empty()) {
            notes.push(detail.clone());
        }
        if pinned.contains(&item.id) {
            notes.push("TOP に固定中".into());
        }
        // How a download or update windows-link runs is going (DLsite).
        if let Some(status) = item.status.as_ref().filter(|s| !s.is_empty()) {
            notes.push(status.clone());
        } else if item.installed != Some(true) {
            notes.push("未インストール（押すとインストール）".into());
        }
        (!notes.is_empty()).then(|| notes.join("・"))
    }

    fn chip(id: &str, name: SharedString, on: bool, theme: &Theme) -> gpui::Stateful<gpui::Div> {
        div()
            .id(SharedString::from(format!("chip-{id}")))
            .debug_selector(|| format!("chip-{name}"))
            .role(Role::Button)
            .aria_label(name.clone())
            .aria_toggled(on.into())
            .focusable()
            .tab_stop(true)
            .focus_visible({
                let c = theme.accent;
                move |s| s.border_color(c)
            })
            .flex()
            .items_center()
            .gap(SP_2)
            .h(CONTROL_HEIGHT)
            .px(SP_5)
            .border_1()
            .rounded(px(28.))
            .text_size(FS_LABEL)
            .cursor_pointer()
            .when(on, |chip| {
                chip.border_color(theme.accent)
                    .bg(theme.accent_subtle)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(glyph(Glyph::Check, px(22.), theme.accent))
            })
            .when(!on, |chip| {
                let hover = theme.hover_1;
                chip.border_color(theme.border)
                    .bg(theme.surface_raised)
                    .hover(move |s| s.bg(hover))
            })
            .child(name)
    }
}

impl Render for LibraryModal {
    #[allow(clippy::too_many_lines)] // One dialog, top to bottom.
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let entity = cx.entity().downgrade();
        let close: Act = {
            let entity = entity.clone();
            Rc::new(move |_, cx| {
                let _ = entity.update(cx, |_, cx| cx.emit(DismissEvent));
            })
        };
        let state = self.state(cx).unwrap_or_default();
        let link = self.link.read(cx);
        let disabled = !link.connected();
        let order = self.order(cx);
        let offered = self
            .library
            .as_ref()
            .map(|l| orders_of(&l.items))
            .unwrap_or_default();
        let shown = self.shown(cx);
        let pinned: Vec<String> = state.pins.iter().map(|p| p.id.clone()).collect();
        let whole = state.whole_pictures();
        let locked = self.locked();

        // The grid: as many columns of at least a button's width as fit.
        let inner = window.viewport_size().width * 0.8 - px(2.) - SP_4 * 2.;
        let (count, width) = columns(inner, TILE_WIDTH, SP_3);
        let ids: Vec<String> = shown.iter().map(|i| i.id.clone()).collect();
        let rows = shown.len().div_ceil(count);
        if self.laid_out != (ids.clone(), count) {
            let before = self.list.item_count();
            if before == 0 || self.laid_out.1 != count {
                self.list.reset(rows);
            } else {
                // Keep the scroll position when only the games' labels or notes changed.
                if rows < before {
                    self.list.splice(rows..before, 0);
                } else if rows > before {
                    self.list.splice(before..before, rows - before);
                }
                self.list.remeasure();
            }
            self.laid_out = (ids, count);
        }
        let tiles: Vec<Rc<Tile>> = Vec::new();
        drop(tiles);
        let base = link.base().to_owned();
        let pending = link.pending.clone();
        let failures = link.failures.clone();
        let games = Rc::new(shown);
        let notes: Rc<Vec<Option<String>>> = Rc::new(
            games
                .iter()
                .map(|i| Self::note_of(i, order, &pinned))
                .collect(),
        );
        let button = self.button.clone();
        let link_handle = self.link.clone();
        let pinned_ids = Rc::new(pinned.clone());
        let grid = list(self.list.clone(), {
            let entity = entity.clone();
            move |row, _window, cx| {
                let theme = Theme::get(cx);
                let start = row * count;
                let end = (start + count).min(games.len());
                div()
                    .flex()
                    .gap(SP_3)
                    .pb(SP_3)
                    .children((start..end).map(|index| {
                        let item = games[index].clone();
                        let key = item_key(&button, &item.id);
                        let on_press = {
                            let (entity, item) = (entity.clone(), item.clone());
                            Rc::new(move |window: &mut Window, cx: &mut App| {
                                let _ = entity.update(cx, |this, cx| this.start(&item, window, cx));
                            })
                        };
                        let on_long = {
                            let (entity, item, link, button, pinned) = (
                                entity.clone(),
                                item.clone(),
                                link_handle.clone(),
                                button.clone(),
                                pinned_ids.clone(),
                            );
                            Rc::new(
                                move |at: Point<Pixels>, window: &mut Window, cx: &mut App| {
                                    let game = MenuGame {
                                        id: item.id.clone(),
                                        name: item.name.clone(),
                                        installed: item.installed,
                                        choosable: item.choosable,
                                        page: Some(item.page.is_some()),
                                    };
                                    let menu = game_menu(
                                        entity.clone(),
                                        &link,
                                        &button,
                                        game,
                                        pinned.contains(&item.id),
                                        at,
                                        cx,
                                    );
                                    let _ = entity
                                        .update(cx, |this, cx| this.show_menu(menu, window, cx));
                                },
                            ) as super::widgets::OnLong
                        };
                        // A flex cell stretches its tile to the row's tallest.
                        div().w(width).flex_none().flex().child(
                            Tile {
                                key: format!("list-{key}").into(),
                                label: item.name.clone().into(),
                                face: Face::Game {
                                    cover: Some(
                                        item.image
                                            .clone()
                                            .unwrap_or_else(|| {
                                                picture_url(&base, &button, &item.id)
                                            })
                                            .into(),
                                    ),
                                    whole,
                                    note: notes[index].clone(),
                                },
                                pending: pending.contains(&key),
                                failure: failures.get(&key).cloned(),
                                disabled,
                                inert: false,
                                on_press,
                                on_long: Some(on_long),
                                drag: Some(DraggedGame {
                                    id: item.id.clone(),
                                    name: item.name.clone().into(),
                                }),
                                gray: false,
                            }
                            .render(&theme),
                        )
                    }))
                    .into_any_element()
            }
        })
        .size_full();

        let search = self.search.clone();
        // The field shows that it has the focus, as the browser's outline did.
        let searching = search.read(cx).focus_handle(cx).is_focused(window);
        let first = self.shown(cx).first().cloned();
        let head = div()
            .flex()
            .gap(SP_3)
            .child(
                div()
                    .id("search")
                    .debug_selector(|| "search".into())
                    .role(Role::SearchInput)
                    .aria_label("名前で探す")
                    .flex_1()
                    .min_w_0()
                    .h(CONTROL_HEIGHT)
                    .px(SP_4)
                    .border_1()
                    .border_color(if searching {
                        theme.accent
                    } else {
                        theme.border
                    })
                    .rounded(RADIUS_MD)
                    .bg(theme.surface_raised)
                    .text_size(FS_LABEL)
                    .on_key_down(cx.listener(
                        move |this, event: &gpui::KeyDownEvent, window, cx| {
                            // Enter starts the first game found, so a keyboard needs no pointer.
                            if event.keystroke.key == "enter"
                                && !search.read(cx).is_composing()
                                && let Some(first) = first.clone()
                            {
                                cx.stop_propagation();
                                this.start(&first, window, cx);
                            }
                        },
                    ))
                    .child(self.search.clone()),
            )
            .when(offered.len() > 1, |head| {
                let hover = theme.hover_1;
                head.child(
                    div()
                        .id("order")
                        .debug_selector(|| "order".into())
                        .role(Role::Button)
                        .aria_label(format!("並び順: {}", order.label()))
                        .focusable()
                        .tab_stop(true)
                        .focus_visible({
                            let c = theme.accent;
                            move |s| s.border_color(c)
                        })
                        .flex()
                        .flex_none()
                        .items_center()
                        .h(CONTROL_HEIGHT)
                        .px(SP_4)
                        .border_1()
                        .border_color(theme.border)
                        .rounded(RADIUS_MD)
                        .bg(theme.surface_raised)
                        .text_size(FS_LABEL)
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .child(order.label())
                        .on_click(cx.listener(|this, event: &gpui::ClickEvent, window, cx| {
                            let at = match event {
                                gpui::ClickEvent::Mouse(click) => click.up.position,
                                _ => window.mouse_position(),
                            };
                            this.order_menu(at, window, cx);
                        })),
                )
            })
            .child(close_button(&theme, close.clone()));

        let chips = self.library.clone().map(|library| {
            let unlabeled = self.active.iter().any(|a| a == UNLABELED);
            let mut chips: Vec<gpui::AnyElement> = vec![
                Self::chip(UNLABELED, "ラベル非登録".into(), unlabeled, &theme)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.active = toggle_label(&this.active, UNLABELED);
                        this.list.reset(0);
                        this.laid_out = (Vec::new(), 0);
                        cx.notify();
                    }))
                    .into_any_element(),
            ];
            for label in library.labels {
                let on = self.active.contains(&label.id);
                let toggled = label.id.clone();
                let mut chip = Self::chip(&label.id, label.name.clone().into(), on, &theme)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.active = toggle_label(&this.active, &toggled);
                        this.list.reset(0);
                        this.laid_out = (Vec::new(), 0);
                        cx.notify();
                    }));
                if locked.is_none() {
                    let (accent, hover_2, dropped) = (theme.accent, theme.hover_2, label.clone());
                    chip = chip
                        .drag_over::<DraggedGame>(move |style, _, _, _| {
                            style
                                .border_2()
                                .border_dashed()
                                .border_color(accent)
                                .bg(hover_2)
                        })
                        .on_drop(cx.listener(move |this, game: &DraggedGame, _, cx| {
                            this.drop_game(game, &dropped, cx);
                        }));
                }
                let on_long = {
                    let (entity, label) = (entity.clone(), label.clone());
                    Rc::new(
                        move |at: Point<Pixels>, window: &mut Window, cx: &mut App| {
                            let _ = entity.update(cx, |this, cx| {
                                this.label_menu(label.clone(), at, window, cx);
                            });
                        },
                    ) as super::widgets::OnLong
                };
                chips.push(
                    long_press(
                        SharedString::from(format!("chip-press-{}", label.id)),
                        chip,
                        Some(on_long),
                        Rc::default(),
                    )
                    .into_any_element(),
                );
            }
            chips.push(
                div()
                    .id("add-label")
                    .debug_selector(|| "add-label".into())
                    .role(Role::Button)
                    .aria_label("ラベルを作る")
                    .when_some(
                        locked.clone(),
                        gpui::StatefulInteractiveElement::aria_description,
                    )
                    .focusable()
                    .tab_stop(locked.is_none())
                    .focus_visible({
                        let c = theme.accent;
                        move |s| s.border_color(c)
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(CONTROL_HEIGHT)
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(28.))
                    .bg(theme.surface_raised)
                    .child(glyph(Glyph::Plus, px(28.), theme.on_surface))
                    .when(locked.is_some(), |b| b.opacity(0.5))
                    .when(locked.is_none(), |b| {
                        let hover = theme.hover_1;
                        b.cursor_pointer().hover(move |s| s.bg(hover)).on_click(
                            cx.listener(|this, _, window, cx| this.ask_create(window, cx)),
                        )
                    })
                    .into_any_element(),
            );
            div()
                .id("chips")
                .role(Role::Group)
                .aria_label("ラベルで絞り込む")
                .flex()
                .flex_wrap()
                .gap(SP_2)
                .children(chips)
        });

        let sign_in = self.library.as_ref().and_then(|l| l.sign_in.clone()).map(|shop| {
            div()
                .id("sign-in")
                .role(Role::Alert)
                .flex()
                .items_center()
                .gap(SP_3)
                .p(SP_3)
                .rounded(RADIUS_SM)
                .bg(theme.danger_subtle)
                .text_color(theme.danger)
                .text_size(FS_SM)
                .child(super::widgets::text(format!(
                    "{} にログインしていません。ログインすると、買ったゲームの一覧とダウンロードが始まります（ログイン画面はメインモニターに開きます）。",
                    shop_name(&shop)
                )).flex_1())
                .child({
                    let busy = self.signing_in || disabled;
                    super::widgets::outlined_button("sign-in-button", "ログイン", theme.danger, &theme)
                        .debug_selector(|| "sign-in".into())
                        .when(self.signing_in, |b| b.child(spinner(theme.accent)))
                        .when(busy, |b| b.opacity(0.5))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !busy {
                                this.sign_in(cx);
                            }
                        }))
                })
        });
        let partial = partial_reason(self.library.as_ref().and_then(|l| l.partial.as_deref()));

        let content = if let Some(failure) = self.failure.clone() {
            super::widgets::text(failure)
                .text_color(theme.danger)
                .into_any_element()
        } else if self.library.is_none() {
            div()
                .flex()
                .items_center()
                .gap(SP_2)
                .text_color(theme.muted)
                .child(spinner(theme.accent))
                .child("読み込んでいます…")
                .into_any_element()
        } else if self.laid_out.0.is_empty() {
            super::widgets::text("見つかりません")
                .text_color(theme.muted)
                .into_any_element()
        } else {
            grid.into_any_element()
        };

        let panel = div()
            .image_cache(self.images.clone())
            .id("library")
            .track_focus(&self.focus)
            .relative()
            .w(relative(0.8))
            .h(relative(0.8))
            .p(SP_4)
            .flex()
            .flex_col()
            .gap(SP_3)
            .bg(theme.surface)
            .child(head)
            .children(chips)
            .when_some(sign_in, gpui::ParentElement::child)
            .when(
                self.library.as_ref().is_some_and(|l| l.sign_in.is_none()),
                |p| {
                    p.when_some(partial, |p, partial| {
                        p.child(
                            super::widgets::text(partial)
                                .text_size(FS_SM)
                                .text_color(theme.muted),
                        )
                    })
                },
            )
            .child(
                div()
                    .id("games")
                    .debug_selector(|| "games-area".into())
                    .flex_1()
                    .min_h_0()
                    .child(content),
            )
            .child(self.notice.render(&theme));
        let menu = self.menu.as_ref().map(|menu| {
            let entity = entity.clone();
            let close: Act = Rc::new(move |window, cx| {
                let _ = entity.update(cx, |this, cx| {
                    this.menu = None;
                    window.focus(&this.search.read(cx).focus_handle(cx), cx);
                    cx.notify();
                });
            });
            menu.render(&theme, &close)
        });
        let dialog = self.dialog.as_ref().map(|(view, _, _)| view.clone());
        // Over the whole panel, with its own menus and dialogs over it.
        div()
            .absolute()
            .inset_0()
            .child(modal(
                "library-layer",
                self.label.clone(),
                &theme,
                close,
                panel,
            ))
            .children(menu)
            .children(dialog)
    }
}
