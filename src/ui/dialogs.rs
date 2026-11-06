//! The dialogs over the panel and the library: names and confirmations, a game's menu, its
//! labels, its program, its license keys, and how a shop's round goes.

use std::{rc::Rc, time::Duration};

use gpui::{
    AnyView, App, AppContext as _, ClipboardItem, Context, DismissEvent, Entity, EventEmitter,
    FocusHandle, Focusable, FontWeight, IntoElement, ParentElement, Point, Pixels, Render, Role,
    SharedString, Styled, Subscription, Task, WeakEntity, Window, div, prelude::*, px,
};

use super::{
    icons::{glyph, spinner},
    text_input::{Changed, TextInput},
    theme::{CONTROL_HEIGHT, FS_LABEL, FS_SM, MONO, RADIUS_MD, ROW_HEIGHT, SP_2, SP_3, SP_4, Theme},
    widgets::{Menu, MenuItem, Notice, dialog_head, modal, outlined_button},
};
use crate::{
    link::{Link, Started, item_key},
    model::{
        display::{Glyph, round_summary},
        library::labels_locked_reason,
        protocol::{Library, LicenseKey, Programs, Round},
    },
};

/// Where a dialog opens and reports back: the panel or the library list.
pub trait Host: 'static + Sized {
    fn open_dialog(&mut self, view: AnyView, focus: FocusHandle, subscription: Subscription, window: &mut Window, cx: &mut Context<Self>);
    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn tell(&mut self, text: String, cx: &mut Context<Self>);
    /// The game's labels or pin changed; read the list again.
    fn changed(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
}

/// Open `view` in `host`, closing it on its [`DismissEvent`].
pub fn open<H: Host, V: Render + Focusable + EventEmitter<DismissEvent>>(
    host: &mut H,
    view: Entity<V>,
    window: &mut Window,
    cx: &mut Context<H>,
) {
    let focus = view.read(cx).focus_handle(cx);
    let subscription = cx.subscribe_in(&view, window, |host: &mut H, _, _: &DismissEvent, window, cx| {
        host.close_dialog(window, cx);
    });
    host.open_dialog(view.into(), focus.clone(), subscription, window, cx);
    window.focus(&focus, cx);
}

fn closer<V: EventEmitter<DismissEvent>>(cx: &Context<V>) -> Rc<dyn Fn(&mut Window, &mut App)> {
    let this = cx.entity().downgrade();
    Rc::new(move |_, cx| {
        let _ = this.update(cx, |_, cx| cx.emit(DismissEvent));
    })
}

/// A game as its menu needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuGame {
    pub id: String,
    pub name: String,
    pub installed: Option<bool>,
    pub choosable: bool,
    /// Whether it has a shop page; none for a pinned game, which goes by its library.
    pub page: Option<bool>,
}

/// A game's long-press menu: pin, its folder, its shop page, its labels, its program and its
/// license keys, as its library offers them.
pub fn game_menu<H: Host>(
    host: WeakEntity<H>,
    link: &Entity<Link>,
    button: &str,
    game: MenuGame,
    pinned: bool,
    at: Point<Pixels>,
    cx: &App,
) -> Menu {
    let library = link.read(cx).buttons.iter().find(|b| b.id == button).and_then(|b| match &b.state {
        crate::model::protocol::State::Library(library) => Some(library.clone()),
        _ => None,
    });
    let keys = library.as_ref().is_some_and(|l| l.license_keys);
    let page = game.page.unwrap_or_else(|| library.as_ref().is_some_and(|l| l.pages));
    let mut items = Vec::new();
    {
        let (host, link, button, game) = (host.clone(), link.clone(), button.to_owned(), game.clone());
        items.push(MenuItem::new(if pinned { "TOPから外す" } else { "TOPに固定" }, move |_, cx| {
            let task = link.update(cx, |l, cx| l.set_pinned(&button, &game.id, !pinned, cx));
            let (host, name) = (host.clone(), game.name.clone());
            cx.spawn(async move |cx| {
                // A failure stays on the game's tile (Link keeps it).
                if task.await.is_none() {
                    let text = format!("「{name}」を TOP {}", if pinned { "から外しました" } else { "に固定しました" });
                    let _ = host.update(cx, |host, cx| host.tell(text, cx));
                }
            })
            .detach();
        }));
    }
    {
        let (link, button, id) = (link.clone(), button.to_owned(), game.id.clone());
        items.push(
            MenuItem::new("ローカルファイル閲覧", move |_, cx| {
                let task = link.update(cx, |l, cx| l.open_folder(&button, &id, cx));
                keep_failure(&link, item_key(&button, &id), task, cx);
            })
            .disabled(game.installed == Some(false)),
        );
    }
    if page {
        let (link, button, id) = (link.clone(), button.to_owned(), game.id.clone());
        items.push(MenuItem::new("販売ページを開く", move |_, cx| {
            let task = link.update(cx, |l, cx| l.open_page(&button, &id, cx));
            keep_failure(&link, item_key(&button, &id), task, cx);
        }));
    }
    {
        let (host, link, button, game) = (host.clone(), link.clone(), button.to_owned(), game.clone());
        items.push(MenuItem::new("ラベル設定", move |window, cx| {
            let (link, button, game) = (link.clone(), button.clone(), game.clone());
            let changed = host.clone();
            let _ = host.update(cx, |host, cx| {
                let picker = cx.new(|cx| LabelPicker::new(link, button, game, changed, window, cx));
                open(host, picker, window, cx);
            });
        }));
    }
    if game.choosable {
        let (host, link, button, game) = (host.clone(), link.clone(), button.to_owned(), game.clone());
        items.push(MenuItem::new("起動ファイルを選ぶ", move |window, cx| {
            let (link, button, game) = (link.clone(), button.clone(), game.clone());
            let _ = host.update(cx, |host, cx| {
                let chooser = cx.new(|cx| ProgramChooser::new(link, button, game.id, game.name, None, cx));
                open(host, chooser, window, cx);
            });
        }));
    }
    if keys {
        let (host, link, button, game) = (host, link.clone(), button.to_owned(), game.clone());
        items.push(MenuItem::new("シリアル番号", move |window, cx| {
            let (link, button, game) = (link.clone(), button.clone(), game.clone());
            let _ = host.update(cx, |host, cx| {
                let keys = cx.new(|cx| KeysDialog::new(link, button, game.id, game.name, cx));
                open(host, keys, window, cx);
            });
        }));
    }
    Menu::new(at, game.name, items, cx)
}

/// Keep why a request about a game failed on its tile.
fn keep_failure(link: &Entity<Link>, key: String, task: Task<Option<String>>, cx: &mut App) {
    let link = link.downgrade();
    cx.spawn(async move |cx| {
        if let Some(failed) = task.await {
            let _ = link.update(cx, |link, cx| {
                link.failures.insert(key, failed);
                cx.notify();
            });
        }
    })
    .detach();
}

fn note<T: Into<SharedString>>(text: T, theme: &Theme) -> impl IntoElement + use<T> {
    div()
        .flex()
        .items_center()
        .gap(SP_2)
        .text_size(FS_SM)
        .text_color(theme.muted)
        .child(text.into())
}

fn loading(text: &'static str, theme: &Theme) -> impl IntoElement + use<> {
    div()
        .id("loading")
        .role(Role::Status)
        .flex()
        .items_center()
        .gap(SP_2)
        .text_size(FS_SM)
        .text_color(theme.muted)
        .child(spinner(theme.accent))
        .child(text)
}

fn failure<T: Into<SharedString>>(text: T, theme: &Theme) -> impl IntoElement + use<T> {
    let text = text.into();
    div()
        .id("failure")
        .role(Role::Alert)
        .aria_label(text.clone())
        .text_size(FS_SM)
        .text_color(theme.danger)
        .child(text)
}

/// A row to choose (a label's check, a program's mark): as tall as a tab.
fn choice_row(id: impl Into<gpui::ElementId>, label: SharedString, theme: &Theme) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .aria_label(label)
        .focusable()
        .tab_stop(true)
        .flex()
        .items_center()
        .gap(SP_3)
        .min_h(ROW_HEIGHT)
        .px(SP_2)
        .text_size(FS_LABEL)
        .rounded(RADIUS_MD)
        .cursor_pointer()
        .hover({ let c = theme.hover_1; move |s| s.bg(c) })
}

/// A 28px mark: a check box, or a round radio mark.
fn mark(on: bool, round: bool, theme: &Theme) -> impl IntoElement + use<> {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.))
        .border_2()
        .border_color(if on { theme.accent } else { theme.border })
        .when(round, |m| m.rounded_full())
        .when(!round, |m| m.rounded(px(6.)))
        .when(on, |m| m.bg(theme.accent_subtle).child(glyph(Glyph::Check, px(20.), theme.accent)))
}

/// Asks for a name (with `value`), or asks to confirm.
pub struct AskDialog {
    title: SharedString,
    message: Option<SharedString>,
    input: Option<Entity<TextInput>>,
    confirm: SharedString,
    danger: bool,
    on_confirm: Rc<dyn Fn(String, &mut Window, &mut App)>,
    focus: FocusHandle,
    _changes: Option<Subscription>,
}

impl EventEmitter<DismissEvent> for AskDialog {}

impl Focusable for AskDialog {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input
            .as_ref()
            .map_or_else(|| self.focus.clone(), |input| input.read(cx).focus_handle(cx))
    }
}

impl AskDialog {
    pub fn new(
        title: impl Into<SharedString>,
        message: Option<&str>,
        value: Option<&str>,
        confirm: impl Into<SharedString>,
        danger: bool,
        on_confirm: impl Fn(String, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = title.into();
        let input = value.map(|value| cx.new(|cx| TextInput::new(title.clone(), value, cx)));
        let changes = input.as_ref().map(|input| cx.subscribe(input, |_, _, _: &Changed, cx| cx.notify()));
        Self {
            title,
            message: message.map(|m| m.to_owned().into()),
            input,
            confirm: confirm.into(),
            danger,
            on_confirm: Rc::new(on_confirm),
            focus: cx.focus_handle(),
            _changes: changes,
        }
    }

    fn text(&self, cx: &App) -> String {
        self.input.as_ref().map(|i| i.read(cx).text().trim().to_owned()).unwrap_or_default()
    }

    fn ready(&self, cx: &App) -> bool {
        self.input.is_none() || !self.text(cx).is_empty()
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ready(cx) {
            return;
        }
        let text = self.text(cx);
        cx.emit(DismissEvent);
        (self.on_confirm)(text, window, cx);
    }
}

impl Render for AskDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let close = closer(cx);
        let ready = self.ready(cx);
        let color = if self.danger { theme.danger } else { theme.accent };
        let composing = self.input.clone();
        let panel = div()
            .id("ask")
            .key_context("AskDialog")
            .track_focus(&self.focus)
            .w(px(480.))
            .max_w_full()
            .p(SP_4)
            .flex()
            .flex_col()
            .gap(SP_3)
            .bg(theme.surface_raised)
            .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                let composing = composing.as_ref().is_some_and(|i| i.read(cx).is_composing());
                if event.keystroke.key == "enter" && !composing {
                    cx.stop_propagation();
                    this.submit(window, cx);
                }
            }))
            .child(div().text_size(FS_LABEL).font_weight(FontWeight::SEMIBOLD).child(self.title.clone()))
            .when_some(self.message.clone(), |panel, message| panel.child(note(message, &theme)))
            .when_some(self.input.clone(), |panel, input| {
                panel.child(
                    div()
                        .h(CONTROL_HEIGHT)
                        .px(SP_4)
                        .border_1()
                        .border_color(theme.border)
                        .rounded(RADIUS_MD)
                        .bg(theme.surface)
                        .text_size(FS_LABEL)
                        .child(input),
                )
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(SP_3)
                    .child(outlined_button("cancel", "やめる", theme.border, &theme).text_color(theme.on_surface).on_click({
                        let close = close.clone();
                        move |_, window, cx| close(window, cx)
                    }))
                    .child(
                        outlined_button("confirm", self.confirm.clone(), color, &theme)
                            .when(!ready, |b| b.opacity(0.5))
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                    ),
            );
        modal("ask-layer", self.title.clone(), &theme, close, panel)
    }
}

/// Choose which program starts a game; with `start`, start it once chosen.
pub struct ProgramChooser {
    link: Entity<Link>,
    button: String,
    id: String,
    name: String,
    /// Start the game once chosen, then call this with its name.
    start: Option<Rc<dyn Fn(String, &mut App)>>,
    programs: Option<Programs>,
    failure: Option<String>,
    busy: bool,
    focus: FocusHandle,
    _load: Task<()>,
}

impl EventEmitter<DismissEvent> for ProgramChooser {}

impl Focusable for ProgramChooser {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ProgramChooser {
    pub fn new(
        link: Entity<Link>,
        button: String,
        id: String,
        name: String,
        start: Option<Rc<dyn Fn(String, &mut App)>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let read = link.update(cx, |l, cx| l.programs(&button, &id, cx));
        let load = cx.spawn(async move |this, cx| {
            let read = read.await;
            let _ = this.update(cx, |this, cx| {
                match read {
                    Ok(programs) => this.programs = Some(programs),
                    Err(failure) => this.failure = Some(failure),
                }
                cx.notify();
            });
        });
        Self { link, button, id, name, start, programs: None, failure: None, busy: false, focus: cx.focus_handle(), _load: load }
    }

    fn choose(&mut self, program: String, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        cx.notify();
        let (link, button, id, name, start) = (self.link.clone(), self.button.clone(), self.id.clone(), self.name.clone(), self.start.clone());
        let chosen = link.update(cx, |l, cx| l.choose_program(&button, &id, &program, cx));
        cx.spawn(async move |this, cx| {
            let failure = chosen.await;
            let started = if failure.is_none() && start.is_some() {
                let task = link.update(cx, |l, cx| l.start_item(&button, &id, cx));
                Some(task.await)
            } else {
                None
            };
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.failure = failure.clone();
                cx.notify();
                if failure.is_none() {
                    cx.emit(DismissEvent);
                    if let (Some(Started::Started), Some(start)) = (started, start) {
                        start(name, cx);
                    }
                }
            });
        })
        .detach();
    }
}

impl Render for ProgramChooser {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let close = closer(cx);
        let rows = self.programs.clone().map(|programs| {
            programs.candidates.into_iter().enumerate().map(|(index, program)| {
                let on = programs.chosen.as_deref() == Some(program.as_str());
                let chosen = program.clone();
                choice_row(("program", index), program.clone().into(), &theme)
                    .role(Role::RadioButton)
                    .aria_toggled(on.into())
                    .when(self.busy, |row| row.opacity(0.5))
                    .on_click(cx.listener(move |this, _, _, cx| this.choose(chosen.clone(), cx)))
                    .child(mark(on, true, &theme))
                    .child(program)
            }).collect::<Vec<_>>()
        });
        let panel = div()
            .id("programs")
            .track_focus(&self.focus)
            .w(px(640.))
            .max_w_full()
            .p(SP_4)
            .flex()
            .flex_col()
            .gap(SP_2)
            .bg(theme.surface_raised)
            .child(dialog_head(format!("起動ファイルを選ぶ: {}", self.name), &theme, close.clone()))
            .child(note("選んだファイルを覚えて、次からはそれで起動します。", &theme))
            .when_some(self.failure.clone(), |p, f| p.child(failure(f, &theme)))
            .when(self.programs.is_none() && self.failure.is_none(), |p| p.child(loading("読み込んでいます…", &theme)))
            .when_some(rows, |p, rows| p.child(div().flex().flex_col().children(rows)));
        modal("programs-layer", "起動ファイルを選ぶ", &theme, close, panel)
    }
}

/// A game's license keys, read from DLsite, to copy into the game.
pub struct KeysDialog {
    name: String,
    keys: Option<Vec<LicenseKey>>,
    failure: Option<String>,
    notice: Notice,
    focus: FocusHandle,
    _load: Task<()>,
}

impl EventEmitter<DismissEvent> for KeysDialog {}

impl Focusable for KeysDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl KeysDialog {
    pub fn new(link: Entity<Link>, button: String, id: String, name: String, cx: &mut Context<Self>) -> Self {
        let read = link.update(cx, |l, cx| l.license_keys(&button, &id, cx));
        let load = cx.spawn(async move |this, cx| {
            let read = read.await;
            let _ = this.update(cx, |this, cx| {
                match read {
                    Ok(keys) => this.keys = Some(keys),
                    Err(failure) => this.failure = Some(failure),
                }
                cx.notify();
            });
        });
        Self { name, keys: None, failure: None, notice: Notice::default(), focus: cx.focus_handle(), _load: load }
    }
}

impl Render for KeysDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let close = closer(cx);
        let body = if let Some(f) = self.failure.clone() {
            failure(f, &theme).into_any_element()
        } else if let Some(keys) = self.keys.clone() {
            if keys.is_empty() {
                note("このゲームにはシリアル番号がありません", &theme).into_any_element()
            } else {
                div()
                    .flex()
                    .flex_col()
                    .gap(SP_3)
                    .children(keys.into_iter().enumerate().map(|(index, key)| {
                        let value = key.value.clone();
                        div()
                            .flex()
                            .items_center()
                            .gap(SP_4)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(div().text_size(FS_SM).text_color(theme.muted).child(key.label.clone()))
                                    .child(div().font_family(MONO).text_size(px(28.)).child(key.value)),
                            )
                            .child(
                                outlined_button(("copy", index), "コピー", theme.accent, &theme)
                                    .aria_label(format!("{}をコピー", key.label))
                                    .h(ROW_HEIGHT)
                                    .min_w(px(120.))
                                    .bg(theme.accent_subtle)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(value.clone()));
                                        this.notice.show("コピーしました", cx, |this: &mut Self| &mut this.notice);
                                    })),
                            )
                    }))
                    .child(note(self.notice.text.clone().unwrap_or_default(), &theme))
                    .into_any_element()
            }
        } else {
            loading("DLsite から読み込んでいます…", &theme).into_any_element()
        };
        let panel = div()
            .id("keys")
            .track_focus(&self.focus)
            .w(px(720.))
            .max_w_full()
            .p(SP_4)
            .flex()
            .flex_col()
            .gap(SP_3)
            .bg(theme.surface_raised)
            .child(dialog_head(format!("シリアル番号: {}", self.name), &theme, close.clone()))
            .child(body);
        modal("keys-layer", "シリアル番号", &theme, close, panel)
    }
}

/// A game's labels, each with a check to put the game in or take it out.
pub struct LabelPicker<H: Host> {
    link: Entity<Link>,
    button: String,
    game: MenuGame,
    host: WeakEntity<H>,
    library: Option<Library>,
    failure: Option<String>,
    /// The label being changed (`new` while making one).
    busy: Option<String>,
    ask: Option<Entity<AskDialog>>,
    focus: FocusHandle,
    _tasks: Vec<Task<()>>,
    _ask: Option<Subscription>,
}

impl<H: Host> EventEmitter<DismissEvent> for LabelPicker<H> {}

impl<H: Host> Focusable for LabelPicker<H> {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl<H: Host> LabelPicker<H> {
    pub fn new(link: Entity<Link>, button: String, game: MenuGame, host: WeakEntity<H>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut picker = Self { link, button, game, host, library: None, failure: None, busy: None, ask: None, focus: cx.focus_handle(), _tasks: Vec::new(), _ask: None };
        picker.load(cx);
        picker
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let read = self.link.update(cx, |l, cx| l.library(&self.button, cx));
        self._tasks.push(cx.spawn(async move |this, cx| {
            let read = read.await;
            let _ = this.update(cx, |this, cx| {
                match read {
                    Ok(library) => {
                        this.library = Some(library);
                        this.failure = None;
                    }
                    Err(failure) => this.failure = Some(failure),
                }
                cx.notify();
            });
        }));
    }

    fn chosen(&self) -> Vec<String> {
        self.library
            .as_ref()
            .and_then(|l| l.items.iter().find(|i| i.id == self.game.id))
            .map(|i| i.labels.clone())
            .unwrap_or_default()
    }

    fn locked(&self) -> Option<String> {
        labels_locked_reason(self.library.as_ref().and_then(|l| l.labels_locked.as_deref()))
    }

    fn after(&mut self, change: Task<Option<String>>, window: &mut Window, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self._tasks.push(cx.spawn_in(window, async move |this, cx| {
            let failure = change.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = None;
                this.failure = failure.clone();
                if failure.is_none() {
                    this.load(cx);
                    let _ = host.update(cx, |host, cx| host.changed(window, cx));
                }
                cx.notify();
            });
        }));
    }

    fn toggle(&mut self, label: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy.is_some() || self.locked().is_some() {
            return;
        }
        let on = !self.chosen().contains(&label);
        self.busy = Some(label.clone());
        cx.notify();
        let change = self.link.update(cx, |l, cx| l.set_label(&self.button, &label, &self.game.id, on, cx));
        self.after(change, window, cx);
    }

    fn create(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = Some("new".into());
        cx.notify();
        let (link, button, id) = (self.link.clone(), self.button.clone(), self.game.id.clone());
        let made = link.update(cx, |l, cx| l.create_label(&button, &name, cx));
        let change = cx.spawn(async move |_, cx| match made.await {
            Ok(label) => {
                let put = link.update(cx, |l, cx| l.set_label(&button, &label.id, &id, true, cx));
                put.await
            }
            Err(failure) => Some(failure),
        });
        self.after(change, window, cx);
    }

    fn ask_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let ask = cx.new(|cx| {
            AskDialog::new("新しいラベル", None, Some(""), "作って付ける", false, move |name, window, cx| {
                let _ = this.update(cx, |this, cx| this.create(name, window, cx));
            }, cx)
        });
        self._ask = Some(cx.subscribe_in(&ask, window, |this, _, _: &DismissEvent, window, cx| {
            this.ask = None;
            window.focus(&this.focus, cx);
            cx.notify();
        }));
        let focus = ask.read(cx).focus_handle(cx);
        self.ask = Some(ask);
        window.focus(&focus, cx);
        cx.notify();
    }
}

impl<H: Host> Render for LabelPicker<H> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let close = closer(cx);
        let locked = self.locked();
        let chosen = self.chosen();
        let rows = self.library.clone().map(|library| {
            let mut rows: Vec<gpui::AnyElement> = library.labels.into_iter().map(|label| {
                let on = chosen.contains(&label.id);
                let id = label.id.clone();
                let busy = self.busy.as_deref() == Some(label.id.as_str());
                choice_row(SharedString::from(format!("label-{}", label.id)), label.name.clone().into(), &theme)
                    .role(Role::CheckBox)
                    .aria_toggled(on.into())
                    .when(locked.is_some(), |row| row.opacity(0.5))
                    .on_click(cx.listener(move |this, _, window, cx| this.toggle(id.clone(), window, cx)))
                    .child(mark(on, false, &theme))
                    .child(label.name)
                    .when(busy, |row| row.child(spinner(theme.accent)))
                    .into_any_element()
            }).collect();
            rows.push(
                choice_row("new-label", "新しいラベル".into(), &theme)
                    .role(Role::Button)
                    .when(locked.is_some(), |row| row.opacity(0.5))
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this.locked().is_none() {
                            this.ask_name(window, cx);
                        }
                    }))
                    .child(div().flex().flex_none().items_center().justify_center().size(px(28.)).child(glyph(Glyph::Plus, px(24.), theme.on_surface)))
                    .child("新しいラベル")
                    .when(self.busy.as_deref() == Some("new"), |row| row.child(spinner(theme.accent)))
                    .into_any_element(),
            );
            rows
        });
        let panel = div()
            .id("labels")
            .track_focus(&self.focus)
            .w(px(560.))
            .max_w_full()
            .max_h(gpui::relative(0.8))
            .overflow_y_scroll()
            .p(SP_4)
            .flex()
            .flex_col()
            .gap(SP_2)
            .bg(theme.surface_raised)
            .child(dialog_head(format!("ラベル設定: {}", self.game.name), &theme, close.clone()))
            .when_some(locked, |p, locked| p.child(note(locked, &theme)))
            .when_some(self.failure.clone(), |p, f| p.child(failure(f, &theme)))
            .when(self.library.is_none() && self.failure.is_none(), |p| p.child(loading("読み込んでいます…", &theme)))
            .when_some(rows, |p, rows| p.child(div().flex().flex_col().children(rows)));
        div()
            .child(modal("labels-layer", "ラベル設定", &theme, close, panel))
            .children(self.ask.clone())
    }
}

/// How often windows-link is asked again while the round's dialog is open.
const POLL: Duration = Duration::from_secs(1);

/// What a shop's round of downloads and updates is doing, followed while open.
pub struct RoundDialog {
    link: Entity<Link>,
    button: String,
    label: String,
    round: Option<Round>,
    failure: Option<String>,
    focus: FocusHandle,
    _poll: Task<()>,
    _link: Subscription,
}

impl EventEmitter<DismissEvent> for RoundDialog {}

impl Focusable for RoundDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl RoundDialog {
    pub fn new(link: Entity<Link>, button: String, label: String, cx: &mut Context<Self>) -> Self {
        let (reader, id) = (link.clone(), button.clone());
        let poll = cx.spawn(async move |this, cx| {
            loop {
                let read = reader.update(cx, |l, cx| l.library_round(&id, cx));
                let read = read.await;
                if this
                    .update(cx, |this, cx| {
                        match read {
                            Ok(round) => {
                                this.round = Some(round);
                                this.failure = None;
                            }
                            Err(failure) => this.failure = Some(failure),
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
                cx.background_executor().timer(POLL).await;
            }
        });
        let observed = cx.observe(&link, |_, _, cx| cx.notify());
        Self { link, button, label, round: None, failure: None, focus: cx.focus_handle(), _poll: poll, _link: observed }
    }
}

impl Render for RoundDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let close = closer(cx);
        let summary = self.round.as_ref().map(|round| round_summary(round, &chrono::Local));
        let link = self.link.read(cx);
        let pending = link.pending.contains(&self.button);
        let button_failure = link.failures.get(&self.button).cloned();
        let busy = summary.as_ref().is_some_and(|s| s.busy) || pending || !link.connected();
        let body = if let Some(f) = self.failure.clone() {
            failure(f, &theme).into_any_element()
        } else if let (Some(summary), Some(round)) = (summary, self.round.clone()) {
            div()
                .flex()
                .flex_col()
                .gap(SP_3)
                .child(
                    div()
                        .id("state")
                        .role(Role::Status)
                        .aria_label(summary.text.clone())
                        .flex()
                        .items_center()
                        .gap(SP_2)
                        .text_size(FS_LABEL)
                        .when(summary.busy, |s| s.child(spinner(theme.accent)))
                        .child(summary.text),
                )
                .when_some(summary.failure, |b, f| b.child(failure(f, &theme)))
                .when(!round.jobs.is_empty(), |b| {
                    b.child(div().flex().flex_col().children(round.jobs.into_iter().map(|job| {
                        let failed = job.status.contains("失敗");
                        div()
                            .flex()
                            .items_center()
                            .gap(SP_4)
                            .min_h(ROW_HEIGHT)
                            .border_t_1()
                            .border_color(theme.border)
                            .child(div().flex_1().min_w_0().truncate().child(job.name))
                            .child(div().text_size(FS_SM).text_color(if failed { theme.danger } else { theme.muted }).child(job.status))
                    })))
                })
                .into_any_element()
        } else {
            loading("読み込んでいます…", &theme).into_any_element()
        };
        let panel = div()
            .id("round")
            .track_focus(&self.focus)
            .w(px(640.))
            .max_w_full()
            .max_h(gpui::relative(0.8))
            .overflow_y_scroll()
            .p(SP_4)
            .flex()
            .flex_col()
            .gap(SP_3)
            .bg(theme.surface_raised)
            .child(dialog_head(format!("更新の状況: {}", self.label), &theme, close.clone()))
            .child(body)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(SP_3)
                    .when_some(button_failure, |a, f| a.child(div().flex_1().child(failure(f, &theme))))
                    .child(
                        outlined_button("now", "今すぐ更新", theme.accent, &theme)
                            .when(pending, |b| b.child(spinner(theme.accent)))
                            .when(busy, |b| b.opacity(0.5))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !busy {
                                    let button = this.button.clone();
                                    this.link.update(cx, |l, cx| l.update_library(&button, cx)).detach();
                                }
                            })),
                    ),
            );
        modal("round-layer", "更新の状況", &theme, close, panel)
    }
}
