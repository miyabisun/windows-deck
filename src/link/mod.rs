//! The connection to windows-link: the button list and states from the `/events` socket,
//! requests over HTTP, and automatic reconnection. Every request runs off the UI thread; a
//! failure stays where the panel shows it until the next try.

pub mod http;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use futures::{StreamExt, channel::mpsc};
use gpui::{AppContext, Context, EventEmitter, Task};
use serde_json::{Value, json};

use crate::model::{
    display::{
        Status, keys_failure, label_failure, library_failure, mixer_failure, press_failure,
        rest_failure, sign_in_failure, switch_failure, update_failure, update_result,
    },
    library::encode,
    placement::{Placement, placement},
    protocol::{
        Button, Desktop, ErrorBody, Event, Label, Library, LicenseKey, Mixer, Monitor, Programs,
        Round,
    },
};
pub use http::{Answer, Connection, Events, Http, SocketMessage};

/// The `pending` and `failures` key of a library game.
pub fn item_key(id: &str, item: &str) -> String {
    format!("{id}/{item}")
}

const FIRST_RETRY: Duration = Duration::from_secs(1);
const MAX_RETRY: Duration = Duration::from_secs(5);

/// Opens a shop's login window and waits until the user has signed in.
pub trait SignIn: Send + Sync + 'static {
    /// Blocking. The shop's cookies (a JSON array for `PUT /{shop}/session`), or why not:
    /// `closed` when the window was closed first.
    fn sign_in(&self, shop: &str, base: &str) -> Result<Value, ErrorBody>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestStep {
    Rest,
    Wake,
}

/// What a game's start came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Started {
    Started,
    /// It has several programs and none is chosen yet.
    Choose,
    Failed,
}

/// Raised when the window must be pinned again: on each new connection (windows-link may have
/// restarted) and when windows-link reconnects to Explorer, which forgets pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinAgain;

/// A change to a volume: its level, its mute, or both.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VolumeChange {
    pub volume: Option<f64>,
    pub muted: Option<bool>,
}

impl VolumeChange {
    fn body(self) -> Value {
        let mut body = serde_json::Map::new();
        if let Some(volume) = self.volume {
            body.insert("volume".into(), json!(volume));
        }
        if let Some(muted) = self.muted {
            body.insert("muted".into(), json!(muted));
        }
        Value::Object(body)
    }
}

pub struct Link {
    base: String,
    http: Arc<dyn Http>,
    events: Arc<dyn Events>,
    login: Arc<dyn SignIn>,
    pub status: LinkStatus,
    pub buttons: Vec<Button>,
    pub pending: HashSet<String>,
    pub failures: HashMap<String, String>,
    pub desktops: Vec<Desktop>,
    /// Desktop files whose desktop does not exist.
    pub unmatched: Vec<String>,
    /// Why windows-link has no desktops, if it says.
    pub desktops_error: Option<String>,
    /// The desktop a tab is switching to.
    pub switching: Option<String>,
    pub desktop_failure: Option<String>,
    /// How many times the window was to be pinned; see [`PinAgain`].
    pub pin_round: u64,
    /// Whether windows-link has darkened every monitor but the panel's.
    pub resting: bool,
    pub rest_pending: Option<RestStep>,
    pub rest_failure: Option<String>,
    connection: Option<Box<dyn Connection>>,
    socket: Option<Task<()>>,
    stopped: bool,
}

impl EventEmitter<PinAgain> for Link {}

/// The answer's status and error body as the failure messages take them.
fn failure_of(answer: Option<&Answer>) -> (Status, Option<ErrorBody>) {
    match answer {
        None => (None, None),
        Some(answer) => (
            Some(answer.status),
            answer
                .body
                .clone()
                .and_then(|body| serde_json::from_value(body).ok()),
        ),
    }
}

/// The answer's body as `T`, when it succeeded and reads as one.
fn success<T: serde::de::DeserializeOwned>(answer: Option<&Answer>) -> Option<T> {
    let answer = answer.filter(|a| a.ok())?;
    serde_json::from_value(answer.body.clone().unwrap_or(Value::Null)).ok()
}

impl Link {
    /// `base` is windows-link's URL, such as `http://127.0.0.1:4730`.
    pub fn new(
        base: &str,
        http: Arc<dyn Http>,
        events: Arc<dyn Events>,
        login: Arc<dyn SignIn>,
    ) -> Self {
        Self {
            base: base.trim_end_matches('/').to_owned(),
            http,
            events,
            login,
            status: LinkStatus::Connecting,
            buttons: Vec::new(),
            pending: HashSet::new(),
            failures: HashMap::new(),
            desktops: Vec::new(),
            unmatched: Vec::new(),
            desktops_error: None,
            switching: None,
            desktop_failure: None,
            pin_round: 0,
            resting: false,
            rest_pending: None,
            rest_failure: None,
            connection: None,
            socket: None,
            stopped: false,
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn connected(&self) -> bool {
        self.status == LinkStatus::Connected
    }

    /// Connect to `/events` and keep reconnecting, waiting longer each time, until stopped.
    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.stopped = false;
        let events = self.events.clone();
        let url = format!(
            "{}/events",
            self.base
                .replacen("https://", "wss://", 1)
                .replacen("http://", "ws://", 1)
        );
        self.socket = Some(cx.spawn(async move |this, cx| {
            let mut retry = FIRST_RETRY;
            loop {
                let (sink, mut messages) = mpsc::unbounded();
                let connection = events.open(&url, sink);
                if this
                    .update(cx, |link, _| link.connection = Some(connection))
                    .is_err()
                {
                    return;
                }
                while let Some(message) = messages.next().await {
                    match message {
                        SocketMessage::Opened => retry = FIRST_RETRY,
                        SocketMessage::Text(text) => {
                            if let Some(event) = Event::parse(&text) {
                                let _ = this.update(cx, |link, cx| link.receive(event, cx));
                            }
                        }
                        SocketMessage::Closed => break,
                    }
                }
                let stopped = this
                    .update(cx, |link, cx| {
                        if !link.stopped {
                            link.status = LinkStatus::Disconnected;
                            cx.notify();
                        }
                        link.stopped
                    })
                    .unwrap_or(true);
                if stopped {
                    return;
                }
                cx.background_executor().timer(retry).await;
                retry = (retry * 2).min(MAX_RETRY);
                if this.read_with(cx, |link, _| link.stopped).unwrap_or(true) {
                    return;
                }
            }
        }));
    }

    pub fn stop(&mut self) {
        self.stopped = true;
        if let Some(connection) = self.connection.take() {
            connection.stop();
        }
        self.socket = None;
    }

    fn receive(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Snapshot {
                buttons,
                desktops,
                unmatched,
                desktops_error,
            } => {
                self.buttons = buttons;
                self.desktops = desktops;
                self.unmatched = unmatched;
                self.desktops_error = desktops_error;
                self.status = LinkStatus::Connected;
                self.pin_again(cx);
            }
            Event::Button(button) => self.replace(button),
            Event::Desktops {
                desktops,
                unmatched,
                error,
                reason,
            } => {
                self.desktops = desktops;
                self.unmatched = unmatched;
                self.desktops_error = error;
                self.desktop_failure = None;
                if reason.as_deref() == Some("reconnected") {
                    self.pin_again(cx);
                }
            }
            Event::Power { resting } => {
                if resting != self.resting {
                    // Rested or woken elsewhere (home-link); a reason from before no longer applies.
                    self.resting = resting;
                    self.rest_failure = None;
                }
            }
        }
        cx.notify();
    }

    fn pin_again(&mut self, cx: &mut Context<Self>) {
        self.pin_round += 1;
        cx.emit(PinAgain);
    }

    fn replace(&mut self, button: Button) {
        if let Some(slot) = self.buttons.iter_mut().find(|b| b.id == button.id) {
            *slot = button;
        }
    }

    fn take_button(&mut self, answer: Option<&Answer>) -> bool {
        #[derive(serde::Deserialize)]
        struct Answered {
            button: Button,
        }
        match success::<Answered>(answer) {
            Some(answered) => {
                self.replace(answered.button);
                true
            }
            None => false,
        }
    }

    /// One request to windows-link, off the UI thread.
    fn request(
        &self,
        method: &'static str,
        path: &str,
        body: Option<Value>,
        cx: &Context<Self>,
    ) -> Task<Option<Answer>> {
        let http = self.http.clone();
        let url = format!("{}{path}", self.base);
        cx.background_spawn(async move { http.call(method, &url, body.as_ref()) })
    }

    fn button_path(id: &str, rest: &str) -> String {
        format!("/buttons/{}{rest}", encode(id))
    }

    fn item_path(id: &str, item: &str, rest: &str) -> String {
        format!("/buttons/{}/library/{}{rest}", encode(id), encode(item))
    }

    /// Press a button; ignored while it is already being pressed or while disconnected.
    pub fn press(&mut self, id: &str, cx: &mut Context<Self>) -> Task<()> {
        if self.pending.contains(id) || !self.connected() {
            return Task::ready(());
        }
        let id = id.to_owned();
        self.pending.insert(id.clone());
        self.failures.remove(&id);
        cx.notify();
        let request = self.request("POST", &Self::button_path(&id, "/press"), None, cx);
        cx.spawn(async move |this, cx| {
            let answer = request.await;
            let _ = this.update(cx, |link, cx| {
                if !link.take_button(answer.as_ref()) {
                    let (status, body) = failure_of(answer.as_ref());
                    link.failures
                        .insert(id.clone(), press_failure(status, body.as_ref()));
                }
                link.pending.remove(&id);
                cx.notify();
            });
        })
    }

    /// Bring a library button's games up to date (windows-link decides how for each library).
    /// Ignored while it is already being updated; a failure stays on the button until its next
    /// update. Resolves to what was started, or none.
    pub fn update_library(&mut self, id: &str, cx: &mut Context<Self>) -> Task<Option<String>> {
        if self.pending.contains(id) || !self.connected() {
            return Task::ready(None);
        }
        let id = id.to_owned();
        self.pending.insert(id.clone());
        self.failures.remove(&id);
        cx.notify();
        let request = self.request("POST", &Self::button_path(&id, "/library/update"), None, cx);
        cx.spawn(async move |this, cx| {
            let answer = request.await;
            this.update(cx, |link, cx| {
                link.pending.remove(&id);
                cx.notify();
                if answer.as_ref().is_some_and(Answer::ok) {
                    return Some(update_result(answer.as_ref().and_then(|a| a.body.as_ref())));
                }
                let (status, body) = failure_of(answer.as_ref());
                link.failures
                    .insert(id.clone(), update_failure(status, body.as_ref()));
                None
            })
            .ok()
            .flatten()
        })
    }

    /// Sign in to a shop (`fanza`, `dlsite`) in the panel's login window, then hand its cookies
    /// to windows-link, which reads the games bought from then on. Resolves to why it did not
    /// go through, or none.
    pub fn sign_in(&self, shop: &str, cx: &Context<Self>) -> Task<Option<String>> {
        let login = self.login.clone();
        let http = self.http.clone();
        let (shop, base) = (shop.to_owned(), self.base.clone());
        cx.background_spawn(async move {
            let cookies = match login.sign_in(&shop, &base) {
                Ok(cookies) => cookies,
                Err(reason) => return Some(sign_in_failure(None, Some(&reason))),
            };
            let url = format!("{base}/{}/session", encode(&shop));
            let answer = http.call("PUT", &url, Some(&json!({ "cookies": cookies })));
            if answer.as_ref().is_some_and(Answer::ok) {
                return None;
            }
            let (status, body) = failure_of(answer.as_ref());
            Some(sign_in_failure(status, body.as_ref()))
        })
    }

    /// A library button's games.
    pub fn library(&self, id: &str, cx: &Context<Self>) -> Task<Result<Library, String>> {
        let request = self.request("GET", &Self::button_path(id, "/library"), None, cx);
        cx.background_spawn(async move {
            let answer = request.await;
            let has_items = answer
                .as_ref()
                .and_then(|a| a.body.as_ref())
                .is_some_and(|b| b.get("items").is_some());
            match success::<Library>(answer.as_ref()) {
                Some(library) if has_items => Ok(library),
                _ => {
                    let (status, body) = failure_of(answer.as_ref());
                    Err(library_failure(status, body.as_ref()))
                }
            }
        })
    }

    /// Start a library game, or bring it to the front when it runs. Ignored while it is already
    /// starting; a failure stays under [`item_key`] until the next try.
    pub fn start_item(&mut self, id: &str, item: &str, cx: &mut Context<Self>) -> Task<Started> {
        let key = item_key(id, item);
        if self.pending.contains(&key) || !self.connected() {
            return Task::ready(Started::Failed);
        }
        self.pending.insert(key.clone());
        self.failures.remove(&key);
        cx.notify();
        let request = self.request("POST", &Self::item_path(id, item, "/start"), None, cx);
        cx.spawn(async move |this, cx| {
            let answer = request.await;
            this.update(cx, |link, cx| {
                link.pending.remove(&key);
                cx.notify();
                if answer.as_ref().is_some_and(Answer::ok) {
                    return Started::Started;
                }
                let (status, body) = failure_of(answer.as_ref());
                if body.as_ref().is_some_and(|b| b.is("choose_program")) {
                    return Started::Choose;
                }
                link.failures
                    .insert(key.clone(), library_failure(status, body.as_ref()));
                Started::Failed
            })
            .unwrap_or(Started::Failed)
        })
    }

    fn read<T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        path: &str,
        failure: fn(Status, Option<&ErrorBody>) -> String,
        cx: &Context<Self>,
    ) -> Task<Result<T, String>> {
        self.send("GET", path, None, failure, cx)
    }

    fn send<T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        method: &'static str,
        path: &str,
        body: Option<Value>,
        failure: fn(Status, Option<&ErrorBody>) -> String,
        cx: &Context<Self>,
    ) -> Task<Result<T, String>> {
        let request = self.request(method, path, body, cx);
        cx.background_spawn(async move {
            let answer = request.await;
            success::<T>(answer.as_ref()).ok_or_else(|| {
                let (status, body) = failure_of(answer.as_ref());
                failure(status, body.as_ref())
            })
        })
    }

    /// A request whose answer has no body to keep; resolves to why it failed, if it did.
    fn act(
        &self,
        method: &'static str,
        path: &str,
        body: Option<Value>,
        failure: fn(Status, Option<&ErrorBody>) -> String,
        cx: &Context<Self>,
    ) -> Task<Option<String>> {
        let request = self.request(method, path, body, cx);
        cx.background_spawn(async move {
            let answer = request.await;
            if answer.as_ref().is_some_and(Answer::ok) {
                return None;
            }
            let (status, body) = failure_of(answer.as_ref());
            Some(failure(status, body.as_ref()))
        })
    }

    /// The programs a library game can start with, and the one in use.
    pub fn programs(
        &self,
        id: &str,
        item: &str,
        cx: &Context<Self>,
    ) -> Task<Result<Programs, String>> {
        self.read(&Self::item_path(id, item, "/programs"), library_failure, cx)
    }

    /// Remember which program starts a game.
    pub fn choose_program(
        &self,
        id: &str,
        item: &str,
        program: &str,
        cx: &Context<Self>,
    ) -> Task<Option<String>> {
        let body = json!({ "program": program });
        self.act(
            "PUT",
            &Self::item_path(id, item, "/program"),
            Some(body),
            library_failure,
            cx,
        )
    }

    /// The default output's volume and the apps with sound on it.
    pub fn mixer(&self, cx: &Context<Self>) -> Task<Result<Mixer, String>> {
        self.read("/audio/mixer", mixer_failure, cx)
    }

    /// Change the default output's volume and/or mute; answers the mixer.
    pub fn set_master(
        &self,
        change: VolumeChange,
        cx: &Context<Self>,
    ) -> Task<Result<Mixer, String>> {
        self.send(
            "PUT",
            "/audio/master",
            Some(change.body()),
            mixer_failure,
            cx,
        )
    }

    /// Change one program's volume and/or its own mute; answers the mixer.
    pub fn set_app(
        &self,
        process: &str,
        change: VolumeChange,
        cx: &Context<Self>,
    ) -> Task<Result<Mixer, String>> {
        let path = format!("/audio/apps/{}", encode(process));
        self.send("PUT", &path, Some(change.body()), mixer_failure, cx)
    }

    /// A game's license keys, read from its store each time.
    pub fn license_keys(
        &self,
        id: &str,
        item: &str,
        cx: &Context<Self>,
    ) -> Task<Result<Vec<LicenseKey>, String>> {
        #[derive(serde::Deserialize)]
        struct Keys {
            keys: Vec<LicenseKey>,
        }
        let read = self.read::<Keys>(&Self::item_path(id, item, "/keys"), keys_failure, cx);
        cx.background_spawn(async move { read.await.map(|k| k.keys) })
    }

    /// Choose the colour a `cursor.highlight` button highlights the cursor in; a failure stays
    /// on the button until its next press or choice.
    pub fn set_color(
        &mut self,
        id: &str,
        color: &str,
        cx: &mut Context<Self>,
    ) -> Task<Option<String>> {
        self.failures.remove(id);
        let id = id.to_owned();
        let request = self.request(
            "PUT",
            &Self::button_path(&id, "/color"),
            Some(json!({ "color": color })),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let answer = request.await;
            this.update(cx, |link, cx| {
                cx.notify();
                if link.take_button(answer.as_ref()) {
                    return None;
                }
                let (status, body) = failure_of(answer.as_ref());
                let failed = press_failure(status, body.as_ref());
                link.failures.insert(id.clone(), failed.clone());
                Some(failed)
            })
            .ok()
            .flatten()
        })
    }

    /// Pin a library game to its button's tab, or take it off.
    pub fn set_pinned(
        &mut self,
        id: &str,
        item: &str,
        pinned: bool,
        cx: &mut Context<Self>,
    ) -> Task<Option<String>> {
        let key = item_key(id, item);
        self.failures.remove(&key);
        let path = format!("/buttons/{}/pins/{}", encode(id), encode(item));
        let request = self.request(if pinned { "PUT" } else { "DELETE" }, &path, None, cx);
        cx.spawn(async move |this, cx| {
            let answer = request.await;
            this.update(cx, |link, cx| {
                cx.notify();
                if link.take_button(answer.as_ref()) {
                    return None;
                }
                let (status, body) = failure_of(answer.as_ref());
                let failed = library_failure(status, body.as_ref());
                link.failures.insert(key.clone(), failed.clone());
                Some(failed)
            })
            .ok()
            .flatten()
        })
    }

    /// How a library's round of downloads and updates goes (`DLsite`, FANZA).
    pub fn library_round(&self, id: &str, cx: &Context<Self>) -> Task<Result<Round, String>> {
        self.read(
            &Self::button_path(id, "/library/update"),
            library_failure,
            cx,
        )
    }

    /// Open a library game's page in its shop, in the default browser.
    pub fn open_page(&self, id: &str, item: &str, cx: &Context<Self>) -> Task<Option<String>> {
        fn failure(status: Status, body: Option<&ErrorBody>) -> String {
            if body.is_some_and(|b| b.is("not_found")) {
                return "このゲームの販売ページは分かりません".into();
            }
            library_failure(status, body)
        }
        self.act(
            "POST",
            &Self::item_path(id, item, "/page"),
            None,
            failure,
            cx,
        )
    }

    /// Show an installed library game's folder in Explorer.
    pub fn open_folder(&self, id: &str, item: &str, cx: &Context<Self>) -> Task<Option<String>> {
        fn failure(status: Status, body: Option<&ErrorBody>) -> String {
            if body.is_some_and(|b| b.is("not_found")) {
                return "このゲームはインストールされていません".into();
            }
            library_failure(status, body)
        }
        self.act(
            "POST",
            &Self::item_path(id, item, "/folder"),
            None,
            failure,
            cx,
        )
    }

    /// Make a label in a library.
    pub fn create_label(
        &self,
        id: &str,
        name: &str,
        cx: &Context<Self>,
    ) -> Task<Result<Label, String>> {
        #[derive(serde::Deserialize)]
        struct Made {
            label: Label,
        }
        let made = self.send::<Made>(
            "POST",
            &Self::button_path(id, "/labels"),
            Some(json!({ "name": name })),
            label_failure,
            cx,
        );
        cx.background_spawn(async move { made.await.map(|m| m.label) })
    }

    pub fn rename_label(
        &self,
        id: &str,
        label: &str,
        name: &str,
        cx: &Context<Self>,
    ) -> Task<Option<String>> {
        let path = Self::button_path(id, &format!("/labels/{}", encode(label)));
        self.act(
            "PATCH",
            &path,
            Some(json!({ "name": name })),
            label_failure,
            cx,
        )
    }

    /// Delete a label; its games stay.
    pub fn delete_label(&self, id: &str, label: &str, cx: &Context<Self>) -> Task<Option<String>> {
        let path = Self::button_path(id, &format!("/labels/{}", encode(label)));
        self.act("DELETE", &path, None, label_failure, cx)
    }

    /// Put a game in a label or take it out.
    pub fn set_label(
        &self,
        id: &str,
        label: &str,
        item: &str,
        on: bool,
        cx: &Context<Self>,
    ) -> Task<Option<String>> {
        let path = Self::button_path(
            id,
            &format!("/labels/{}/items/{}", encode(label), encode(item)),
        );
        self.act(
            if on { "PUT" } else { "DELETE" },
            &path,
            None,
            label_failure,
            cx,
        )
    }

    /// Ask windows-link to switch Windows to a desktop; one switch at a time.
    pub fn switch_desktop(&mut self, id: &str, cx: &mut Context<Self>) -> Task<()> {
        if self.switching.is_some() || !self.connected() {
            return Task::ready(());
        }
        self.switching = Some(id.to_owned());
        self.desktop_failure = None;
        cx.notify();
        let request = self.request(
            "POST",
            &format!("/desktops/{}/switch", encode(id)),
            None,
            cx,
        );
        cx.spawn(async move |this, cx| {
            let answer = request.await;
            let _ = this.update(cx, |link, cx| {
                #[derive(serde::Deserialize)]
                struct Switched {
                    desktops: Vec<Desktop>,
                }
                if let Some(switched) = success::<Switched>(answer.as_ref()) {
                    link.desktops = switched.desktops;
                } else {
                    let (status, body) = failure_of(answer.as_ref());
                    link.desktop_failure = Some(switch_failure(status, body.as_ref()));
                }
                link.switching = None;
                cx.notify();
            });
        })
    }

    /// Show a window on every virtual desktop. Resolves to whether windows-link pinned it.
    pub fn pin(&self, hwnd: isize, cx: &Context<Self>) -> Task<bool> {
        let request = self.request("POST", &format!("/windows/{hwnd}/pin"), None, cx);
        cx.background_spawn(async move { request.await.is_some_and(|a| a.ok()) })
    }

    /// Create a desktop for a desktop file that has none; Windows switches to it.
    pub fn create_desktop(&mut self, name: &str, cx: &mut Context<Self>) -> Task<()> {
        if !self.connected() {
            return Task::ready(());
        }
        self.desktop_failure = None;
        cx.notify();
        let name = name.to_owned();
        let request = self.request("POST", "/desktops", Some(json!({ "name": name })), cx);
        cx.spawn(async move |this, cx| {
            let answer = request.await;
            let _ = this.update(cx, |link, cx| {
                #[derive(serde::Deserialize)]
                struct Created {
                    desktops: Vec<Desktop>,
                    #[serde(default)]
                    unmatched: Vec<String>,
                }
                if let Some(created) = success::<Created>(answer.as_ref()) {
                    link.desktops = created.desktops;
                    link.unmatched = created.unmatched;
                } else {
                    let (status, body) = failure_of(answer.as_ref());
                    link.desktop_failure = Some(if body.as_ref().is_some_and(|b| b.is("exists")) {
                        format!("「{name}」はもうあります")
                    } else {
                        switch_failure(status, body.as_ref())
                    });
                }
                cx.notify();
            });
        })
    }

    /// The monitors windows-link knows, or none without an answer.
    pub fn touch_monitors(&self, cx: &Context<Self>) -> Task<Option<Vec<Monitor>>> {
        let request = self.request("GET", "/touch-monitors", None, cx);
        cx.background_spawn(async move { success(request.await.as_ref()) })
    }

    /// Read whether windows-link is resting; it may have been before this connection.
    pub fn sync_rest(&self, cx: &mut Context<Self>) -> Task<()> {
        let request = self.request("GET", "/power", None, cx);
        cx.spawn(async move |this, cx| {
            #[derive(serde::Deserialize)]
            struct Power {
                resting: bool,
            }
            // Without an answer the next connection reads it again.
            if let Some(power) = success::<Power>(request.await.as_ref()) {
                let _ = this.update(cx, |link, cx| {
                    link.resting = power.resting;
                    cx.notify();
                });
            }
        })
    }

    /// Darken every monitor but the panel's: `monitor` as configured (id, name or display name,
    /// as for placing the window).
    pub fn rest(&mut self, monitor: Option<&str>, cx: &mut Context<Self>) -> Task<()> {
        if self.rest_pending.is_some() || !self.connected() {
            return Task::ready(());
        }
        self.rest_pending = Some(RestStep::Rest);
        self.rest_failure = None;
        cx.notify();
        let listed = self.request("GET", "/touch-monitors", None, cx);
        let (http, base, monitor) = (
            self.http.clone(),
            self.base.clone(),
            monitor.map(str::to_owned),
        );
        cx.spawn(async move |this, cx| {
            let monitors: Option<Vec<Monitor>> = success(listed.await.as_ref());
            let failure = match placement(monitors.as_deref(), monitor.as_deref()) {
                Placement::Fill { id, .. } => {
                    let answer = cx
                        .background_spawn(async move {
                            http.call(
                                "POST",
                                &format!("{base}/power/rest"),
                                Some(&json!({ "keep": id })),
                            )
                        })
                        .await;
                    if answer.as_ref().is_some_and(Answer::ok) {
                        None
                    } else {
                        let (status, body) = failure_of(answer.as_ref());
                        Some(rest_failure(status, body.as_ref()))
                    }
                }
                _ => Some(rest_failure(None, Some(&ErrorBody::named("no_monitor")))),
            };
            let _ = this.update(cx, |link, cx| {
                match failure {
                    None => link.resting = true,
                    Some(failure) => link.rest_failure = Some(failure),
                }
                link.rest_pending = None;
                cx.notify();
            });
        })
    }

    /// Light the monitors again. A brightness windows-link could not put back (`unrestored`,
    /// after trying again itself) leaves it awake, and the panel too, saying which; any other
    /// failure keeps the panel resting, to press again.
    pub fn wake(&mut self, cx: &mut Context<Self>) -> Task<()> {
        if self.rest_pending.is_some() {
            return Task::ready(());
        }
        self.rest_pending = Some(RestStep::Wake);
        self.rest_failure = None;
        cx.notify();
        let request = self.request("POST", "/power/wake", None, cx);
        cx.spawn(async move |this, cx| {
            let answer = request.await;
            let _ = this.update(cx, |link, cx| {
                let ok = answer.as_ref().is_some_and(Answer::ok);
                let (status, body) = failure_of(answer.as_ref());
                if ok || body.as_ref().is_some_and(|b| b.is("unrestored")) {
                    link.resting = false;
                }
                if !ok {
                    link.rest_failure = Some(rest_failure(status, body.as_ref()));
                }
                link.rest_pending = None;
                cx.notify();
            });
        })
    }
}

#[cfg(test)]
mod tests;
