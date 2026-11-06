//! A stand-in for windows-link for the panel's tests: its button API and `/events`, with
//! switches for the tests (the counterpart of the Playwright suite's `e2e/mock-link.js`).

// The stand-in takes its JSON as the tests write it.
#![allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]

use std::sync::{Arc, Mutex, MutexGuard};

use futures::channel::mpsc::UnboundedSender;
use serde_json::{Value, json};

use super::{
    SignIn,
    http::{Answer, Connection, Events, Http, SocketMessage},
};
use crate::model::protocol::ErrorBody;

pub const BASE: &str = "http://127.0.0.1:4730";

pub fn output(current: &str) -> Value {
    let name = if current == "motu" {
        "MOTU (MOTU M Series)"
    } else {
        "BTイヤホン (JBL Tour Pro 3)"
    };
    json!({ "id": "output", "type": "audio.output_toggle", "label": "出力切替", "desktop": null,
        "state": { "kind": "output", "current": current, "current_name": name, "options": [
            { "alias": "motu", "name": "MOTU (MOTU M Series)", "connected": true, "icon": "speaker" },
            { "alias": "jbl", "name": "BTイヤホン (JBL Tour Pro 3)", "connected": true, "icon": "headphones" } ] } })
}

pub fn volume(value: f64) -> Value {
    json!({ "id": "sf6-volume", "type": "audio.app_volume_toggle", "label": "スト6 音量", "desktop": null,
        "state": { "kind": "volume", "running": true, "volume": value, "levels": [0.2, 1] } })
}

pub fn desktops(current: usize) -> Value {
    Value::Array(
        ["dev", "ゲーム", "ブルアカ", "アダルト"]
            .iter()
            .enumerate()
            .map(|(index, name)| json!({ "id": format!("GUID-{index}"), "name": name, "index": index, "current": index == current }))
            .collect(),
    )
}

pub fn library_button(pins: Value) -> Value {
    json!({ "id": "games", "type": "steam.library", "label": "Steam", "desktop": null, "except": [], "icon": false,
        "state": { "kind": "library", "pins": pins } })
}

pub fn highlight_button(color: &str) -> Value {
    json!({ "id": "cursor", "type": "cursor.highlight", "label": "マウスを目立たせる", "desktop": null, "except": [], "icon": false,
        "state": { "kind": "highlight", "on": false, "color": color } })
}

pub fn initial_mixer() -> Value {
    json!({ "master": { "volume": 0.45, "muted": false }, "apps": [
        { "process": "Discord.exe", "name": "Discord", "volume": 1, "muted": false },
        { "process": "StreetFighter6.exe", "name": "StreetFighter6", "volume": 0.2, "muted": false } ] })
}

pub fn mute_button(master: &Value) -> Value {
    let mut state = master.clone();
    state["kind"] = json!("mute");
    json!({ "id": "mute", "type": "audio.mute_toggle", "label": "ミュート", "desktop": null, "except": [], "icon": false, "state": state })
}

pub fn mixer_button(master: &Value) -> Value {
    let mut state = master.clone();
    state["kind"] = json!("mixer");
    json!({ "id": "mixer", "type": "audio.mixer", "label": "ミキサー", "desktop": null, "except": [], "icon": false, "state": state })
}

pub fn app_mute_button(app: Option<&Value>) -> Value {
    json!({ "id": "sf6-mute", "type": "audio.app_mute_toggle", "label": "スト6 音声", "desktop": null, "except": [], "icon": false,
        "state": { "kind": "app_mute", "running": app.is_some(), "muted": app.and_then(|a| a["muted"].as_bool()).unwrap_or(false) } })
}

pub fn dlsite_button(pins: Value) -> Value {
    json!({ "id": "dlsite", "type": "dlsite.library", "label": "DLsite", "desktop": null, "except": [], "icon": false,
        "state": { "kind": "library", "pins": pins, "pictures": "whole", "license_keys": true, "rounds": true, "pages": true } })
}

pub fn dlsite_page(id: &str) -> String {
    format!("https://www.dlsite.com/maniax/work/=/product_id/{id}.html")
}

/// The games behind `dlsite_button`: "a1" starts, "b2" has two programs to choose from, "c3"
/// has none. "a1" (updated 2026/10/16, bought 2026/01/02 in Japan) and "b2" (updated
/// 2026/10/28) are known works with a page; "c3" is not.
pub fn initial_dlsite() -> Value {
    let game = |id: &str, name: &str, detail: &str, choosable: bool, extra: Value| {
        let mut game = json!({ "id": id, "name": name, "detail": detail, "choosable": choosable, "image": null,
            "installed": true, "labels": [], "updated": null, "bought": null, "page": null });
        if let (Some(game), Some(extra)) = (game.as_object_mut(), extra.as_object()) {
            game.extend(extra.clone());
        }
        game
    };
    json!({
        "items": [
            game("a1", "湿度の高い夏のマゾ", "3Djp_Art", false, json!({ "image": format!("{BASE}/dlsite-art/a1.svg"), "updated": 1_792_076_400, "bought": 1_767_290_645, "page": dlsite_page("RJ01") })),
            game("b2", "催眠アプリ", "Saimin Soft", true, json!({ "updated": 1_793_118_930, "page": dlsite_page("RJ02") })),
            game("c3", "壊れたゲーム", "Other", false, json!({})),
        ],
        "labels": [ { "id": "favorite", "name": "お気に入り", "editable": false }, { "id": "hidden", "name": "非表示", "editable": false } ],
        "hide": ["hidden"], "partial": null, "labels_locked": null,
        "programs": { "b2": { "candidates": ["app.exe", "startup.exe"], "chosen": null } },
        "keys": { "a1": [ { "label": "ライセンスキー", "value": "ABCD-1234-EFGH-5678" } ] },
    })
}

/// The games behind `library_button`; labels name their games by ID. A long name and a note
/// (2851970) sit in the same row as short names without one.
pub fn initial_library() -> Value {
    let game = |id: &str, name: &str, labels: &[&str], installed: bool| json!({ "id": id, "name": name, "installed": installed, "labels": labels });
    json!({
        "items": [
            game("1364780", "Street Fighter™ 6", &["favorite"], true),
            game("646570", "Slay the Spire", &[], true),
            game("2851970", "NEEDY GIRL OVERDOSE タイピング オブ ザ ネット", &["favorite"], false),
            game("2868840", "Slay the Spire 2", &["uc-r15"], false),
            game("1039890", "METAL SLUG", &["hidden"], true),
        ],
        "labels": [ { "id": "favorite", "name": "お気に入り", "editable": false }, { "id": "hidden", "name": "非表示", "editable": false }, { "id": "uc-r15", "name": "R15", "editable": true } ],
        "hide": ["hidden"], "partial": null, "labels_locked": null,
    })
}

/// What the stand-in knows and what it was asked.
pub struct State {
    pub buttons: Vec<Value>,
    pub presses: Vec<String>,
    /// The next press's answer, else it presses.
    pub next_failure: Option<(u16, Value)>,
    pub down: bool,
    pub desktops: Value,
    pub desktops_error: Option<String>,
    pub unmatched: Vec<String>,
    pub monitors: Value,
    pub resting: bool,
    pub rests: Vec<String>,
    pub wakes: usize,
    pub wake_answer: Option<(u16, Value)>,
    pub switches: Vec<String>,
    pub pins: Vec<String>,
    pub library: Value,
    pub dlsite: Value,
    pub mixer: Value,
    pub started: Vec<String>,
    pub folders: Vec<String>,
    pub updates: Vec<String>,
    pub update_answer: Option<(u16, Value)>,
    pub dlsite_round: Value,
    pub pages: Vec<String>,
    pub colors: Vec<String>,
    pub sessions: Vec<(String, Value)>,
    /// The login window's answer: the cookies, or why there are none.
    pub login: Result<Value, ErrorBody>,
    sockets: Vec<UnboundedSender<SocketMessage>>,
}

#[derive(Clone)]
pub struct MockLink(Arc<Mutex<State>>);

impl Default for MockLink {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(State {
            buttons: vec![output("motu"), volume(1.)],
            presses: Vec::new(),
            next_failure: None,
            down: false,
            desktops: desktops(0),
            desktops_error: None,
            unmatched: Vec::new(),
            monitors: json!([
                { "id": "mon-main", "name": "25M2S", "gdi_name": "DISPLAY1", "primary": true, "touch": false },
                { "id": "mon-panel", "name": "T10FA", "gdi_name": "DISPLAY5", "primary": false, "touch": true }
            ]),
            resting: false,
            rests: Vec::new(),
            wakes: 0,
            wake_answer: None,
            switches: Vec::new(),
            pins: Vec::new(),
            library: initial_library(),
            dlsite: initial_dlsite(),
            mixer: initial_mixer(),
            started: Vec::new(),
            folders: Vec::new(),
            updates: Vec::new(),
            update_answer: None,
            dlsite_round: json!({ "doing": null, "started": 1_793_118_870, "finished": 1_793_118_930,
                "purchases": "DLsite's login page: http status: 403", "jobs": [] }),
            pages: Vec::new(),
            colors: Vec::new(),
            sessions: Vec::new(),
            login: Err(ErrorBody::named("closed")),
            sockets: Vec::new(),
        })))
    }
}

fn answer(status: u16, body: Value) -> Option<Answer> {
    Some(Answer {
        status,
        body: (!body.is_null()).then_some(body),
    })
}

impl MockLink {
    pub fn state(&self) -> MutexGuard<'_, State> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn broadcast(state: &mut State, message: &Value) {
        let text = message.to_string();
        state.sockets.retain(|socket| {
            socket
                .unbounded_send(SocketMessage::Text(text.clone()))
                .is_ok()
        });
    }

    /// Change a button as if Windows changed it, and notify as windows-link does.
    pub fn change(&self, button: Value) {
        let mut state = self.state();
        if let Some(slot) = state.buttons.iter_mut().find(|b| b["id"] == button["id"]) {
            *slot = button.clone();
        }
        Self::broadcast(&mut state, &json!({ "type": "button", "button": button }));
    }

    /// Rest or wake as if home-link asked, and notify as windows-link does.
    pub fn set_resting(&self, resting: bool) {
        let mut state = self.state();
        state.resting = resting;
        Self::broadcast(&mut state, &json!({ "type": "power", "resting": resting }));
    }

    /// Change the desktops as if Windows changed them, and notify as windows-link does.
    pub fn set_desktops(&self, desktops: Value, reason: &str) {
        let mut state = self.state();
        state.desktops = desktops.clone();
        let unmatched = state.unmatched.clone();
        Self::broadcast(
            &mut state,
            &json!({ "type": "desktops", "reason": reason, "desktops": desktops, "unmatched": unmatched, "error": null }),
        );
    }

    pub fn go_down(&self) {
        let mut state = self.state();
        state.down = true;
        for socket in state.sockets.drain(..) {
            let _ = socket.unbounded_send(SocketMessage::Closed);
        }
    }

    pub fn come_up(&self) {
        self.state().down = false;
    }

    fn master_changed(state: &mut State) {
        let master = state.mixer["master"].clone();
        for index in 0..state.buttons.len() {
            let next = match state.buttons[index]["id"].as_str() {
                Some("mute") => mute_button(&master),
                Some("mixer") => mixer_button(&master),
                _ => continue,
            };
            state.buttons[index] = next.clone();
            Self::broadcast(state, &json!({ "type": "button", "button": next }));
        }
    }

    fn app_changed(state: &mut State) {
        let Some(index) = state.buttons.iter().position(|b| b["id"] == "sf6-mute") else {
            return;
        };
        let app = state.mixer["apps"]
            .as_array()
            .and_then(|apps| apps.iter().find(|a| a["process"] == "StreetFighter6.exe"))
            .cloned();
        let button = app_mute_button(app.as_ref());
        state.buttons[index] = button.clone();
        Self::broadcast(state, &json!({ "type": "button", "button": button }));
    }

    fn button(state: &State, id: &str) -> Value {
        state
            .buttons
            .iter()
            .find(|b| b["id"] == id)
            .cloned()
            .unwrap_or(Value::Null)
    }

    #[allow(clippy::too_many_lines)] // One windows-link, route by route.
    fn route(&self, method: &str, path: &str, body: Option<&Value>) -> Option<Answer> {
        let mut state = self.state();
        let s = &mut *state;
        if s.down && !(method == "GET" && (path == "/touch-monitors" || path == "/power")) {
            return None;
        }
        let parts: Vec<String> = path
            .trim_start_matches('/')
            .split('/')
            .map(|p| {
                percent_encoding::percent_decode_str(p)
                    .decode_utf8_lossy()
                    .into_owned()
            })
            .collect();
        let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
        let field = |name: &str| {
            body.and_then(|b| b.get(name))
                .cloned()
                .unwrap_or(Value::Null)
        };
        match (method, parts.as_slice()) {
            ("POST", ["desktops", id, "switch"]) => {
                s.switches.push((*id).to_owned());
                let known = s
                    .desktops
                    .as_array()
                    .is_some_and(|d| d.iter().any(|d| d["id"] == *id));
                if !known {
                    return answer(
                        404,
                        json!({ "error": "not_found", "message": "no desktop with this id" }),
                    );
                }
                if let Some(list) = s.desktops.as_array_mut() {
                    for desktop in list {
                        desktop["current"] = json!(desktop["id"] == *id);
                    }
                }
                let desktops = s.desktops.clone();
                Self::broadcast(
                    s,
                    &json!({ "type": "desktops", "reason": "changed", "desktops": desktops, "error": null }),
                );
                answer(200, json!({ "desktops": s.desktops, "error": null }))
            }
            ("POST", ["desktops"]) => {
                let name = field("name").as_str().unwrap_or_default().to_owned();
                let list = s.desktops.as_array().cloned().unwrap_or_default();
                if list.iter().any(|d| d["name"] == name.as_str()) {
                    return answer(409, json!({ "error": "exists", "message": "exists" }));
                }
                let mut list: Vec<Value> = list
                    .into_iter()
                    .map(|mut d| {
                        d["current"] = json!(false);
                        d
                    })
                    .collect();
                list.push(json!({ "id": format!("GUID-{}", list.len()), "name": name, "index": list.len(), "current": true }));
                s.desktops = Value::Array(list);
                s.unmatched.retain(|n| *n != name);
                let (desktops, unmatched) = (s.desktops.clone(), s.unmatched.clone());
                Self::broadcast(
                    s,
                    &json!({ "type": "desktops", "reason": "created", "desktops": desktops, "unmatched": unmatched, "error": null }),
                );
                answer(
                    201,
                    json!({ "desktops": s.desktops, "unmatched": s.unmatched, "error": null }),
                )
            }
            ("GET", ["touch-monitors"]) => answer(200, s.monitors.clone()),
            ("GET", ["power"]) => answer(200, json!({ "resting": s.resting })),
            ("POST", ["power", "rest"]) => {
                let keep = field("keep").as_str().unwrap_or_default().to_owned();
                s.rests.push(keep.clone());
                if !s
                    .monitors
                    .as_array()
                    .is_some_and(|m| m.iter().any(|m| m["id"] == keep.as_str()))
                {
                    return answer(
                        404,
                        json!({ "error": "not_found", "message": "no connected monitor" }),
                    );
                }
                s.resting = true;
                Self::broadcast(s, &json!({ "type": "power", "resting": true }));
                answer(200, json!({ "resting": true }))
            }
            ("POST", ["power", "wake"]) => {
                s.wakes += 1;
                let (status, body) = s
                    .wake_answer
                    .take()
                    .unwrap_or((200, json!({ "resting": false })));
                // windows-link is awake after a brightness it could not put back, too.
                if status < 400 || body["error"] == "unrestored" {
                    s.resting = false;
                    Self::broadcast(s, &json!({ "type": "power", "resting": false }));
                }
                answer(status, body)
            }
            ("GET", ["buttons", id, "library", "update"]) => {
                if *id != "dlsite" {
                    return answer(404, json!({ "error": "no_rounds", "message": "no rounds" }));
                }
                answer(200, s.dlsite_round.clone())
            }
            ("POST", ["buttons", id, "library", "update"]) => {
                s.updates.push((*id).to_owned());
                let (status, body) = s
                    .update_answer
                    .take()
                    .unwrap_or((200, json!({ "updates": 3, "installs": 24 })));
                answer(status, body)
            }
            ("GET", ["buttons", "dlsite", "library"]) => {
                let mut listing = s.dlsite.clone();
                if let Some(listing) = listing.as_object_mut() {
                    listing.remove("programs");
                    listing.remove("keys");
                }
                answer(200, listing)
            }
            (_, ["buttons", "dlsite", "library", item, action]) => {
                let item = (*item).to_owned();
                match (method, *action) {
                    ("GET", "keys") => answer(
                        200,
                        json!({ "keys": s.dlsite["keys"][item.as_str()].as_array().cloned().unwrap_or_default() }),
                    ),
                    ("POST", "page") => {
                        let known = s.dlsite["items"]
                            .as_array()
                            .and_then(|i| i.iter().find(|i| i["id"] == item.as_str()))
                            .is_some_and(|i| !i["page"].is_null());
                        if !known {
                            return answer(
                                404,
                                json!({ "error": "not_found", "message": "the game has no page in its shop" }),
                            );
                        }
                        s.pages.push(item);
                        answer(204, Value::Null)
                    }
                    ("GET", "programs") => match s.dlsite["programs"].get(item.as_str()) {
                        Some(programs) => answer(200, programs.clone()),
                        None => answer(404, json!({ "error": "not_found" })),
                    },
                    ("PUT", "program") => {
                        s.dlsite["programs"][item.as_str()]["chosen"] = field("program");
                        answer(204, Value::Null)
                    }
                    ("POST", "start") => {
                        if item == "c3" {
                            return answer(
                                409,
                                json!({ "error": "no_program", "message": "it has no program to start" }),
                            );
                        }
                        let coming = s.dlsite["items"]
                            .as_array()
                            .and_then(|i| {
                                i.iter()
                                    .find(|i| i["id"] == item.as_str() && i["installed"] == false)
                            })
                            .cloned();
                        if let Some(coming) = coming {
                            return answer(
                                409,
                                json!({ "error": "not_downloaded", "message": coming["status"] }),
                            );
                        }
                        let programs = s.dlsite["programs"].get(item.as_str()).cloned();
                        if let Some(programs) = &programs
                            && programs["chosen"].is_null()
                        {
                            return answer(
                                409,
                                json!({ "error": "choose_program", "message": "choose which program starts it" }),
                            );
                        }
                        let started = match programs {
                            Some(programs) => format!(
                                "{item}:{}",
                                programs["chosen"].as_str().unwrap_or_default()
                            ),
                            None => item,
                        };
                        s.started.push(started);
                        answer(204, Value::Null)
                    }
                    _ => answer(404, Value::Null),
                }
            }
            ("GET", ["buttons", "games", "library"]) => {
                let pinned: Vec<Value> = Self::button(s, "games")["state"]["pins"]
                    .as_array()
                    .map(|p| p.iter().map(|p| p["id"].clone()).collect())
                    .unwrap_or_default();
                let mut library = s.library.clone();
                if let Some(items) = library["items"].as_array_mut() {
                    for item in items {
                        item["pinned"] = json!(pinned.contains(&item["id"]));
                    }
                }
                answer(200, library)
            }
            ("POST", ["buttons", "games", "library", item, "start"]) => {
                s.started.push((*item).to_owned());
                answer(204, Value::Null)
            }
            ("PUT", ["buttons", "cursor", "color"]) => {
                let color = field("color").as_str().unwrap_or_default().to_owned();
                s.colors.push(color.clone());
                let button = highlight_button(&color);
                if let Some(slot) = s.buttons.iter_mut().find(|b| b["id"] == "cursor") {
                    *slot = button.clone();
                }
                Self::broadcast(s, &json!({ "type": "button", "button": button }));
                answer(200, json!({ "button": button }))
            }
            ("PUT" | "DELETE", ["buttons", "games", "pins", item]) => {
                let Some(game) = s.library["items"]
                    .as_array()
                    .and_then(|i| i.iter().find(|i| i["id"] == *item))
                    .cloned()
                else {
                    return answer(
                        404,
                        json!({ "error": "not_found", "message": "no such game" }),
                    );
                };
                let mut pins: Vec<Value> = Self::button(s, "games")["state"]["pins"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                pins.retain(|p| p["id"] != game["id"]);
                if method == "PUT" {
                    pins.push(json!({ "id": game["id"], "name": game["name"] }));
                }
                let button = library_button(Value::Array(pins));
                if let Some(slot) = s.buttons.iter_mut().find(|b| b["id"] == "games") {
                    *slot = button.clone();
                }
                Self::broadcast(s, &json!({ "type": "button", "button": button }));
                answer(200, json!({ "button": button }))
            }
            ("POST", ["buttons", "games", "library", item, "folder"]) => {
                let installed = s.library["items"]
                    .as_array()
                    .and_then(|i| i.iter().find(|i| i["id"] == *item))
                    .is_some_and(|i| i["installed"] == true);
                if !installed {
                    return answer(
                        404,
                        json!({ "error": "not_found", "message": "not installed" }),
                    );
                }
                s.folders.push((*item).to_owned());
                answer(204, Value::Null)
            }
            ("POST", ["buttons", "games", "labels"]) => {
                let count = s.library["labels"].as_array().map_or(0, Vec::len);
                let label = json!({ "id": format!("uc-{}", count + 1), "name": field("name"), "editable": true });
                if let Some(labels) = s.library["labels"].as_array_mut() {
                    labels.push(label.clone());
                }
                answer(201, json!({ "label": label }))
            }
            ("PATCH", ["buttons", "games", "labels", id]) => {
                if let Some(label) = s.library["labels"]
                    .as_array_mut()
                    .and_then(|l| l.iter_mut().find(|l| l["id"] == *id))
                {
                    label["name"] = field("name");
                }
                answer(204, Value::Null)
            }
            ("DELETE", ["buttons", "games", "labels", id]) => {
                if let Some(labels) = s.library["labels"].as_array_mut() {
                    labels.retain(|l| l["id"] != *id);
                }
                if let Some(items) = s.library["items"].as_array_mut() {
                    for item in items {
                        if let Some(labels) = item["labels"].as_array_mut() {
                            labels.retain(|l| l != *id);
                        }
                    }
                }
                answer(204, Value::Null)
            }
            ("PUT" | "DELETE", ["buttons", "games", "labels", id, "items", item]) => {
                if let Some(game) = s.library["items"]
                    .as_array_mut()
                    .and_then(|i| i.iter_mut().find(|i| i["id"] == *item))
                    && let Some(labels) = game["labels"].as_array_mut()
                {
                    labels.retain(|l| l != *id);
                    if method == "PUT" {
                        labels.push(json!(id));
                    }
                }
                answer(204, Value::Null)
            }
            ("POST", ["windows", hwnd, "pin"]) => {
                s.pins.push((*hwnd).to_owned());
                answer(200, json!({ "pinned": true }))
            }
            ("POST", ["buttons", "sf6-mute", "press"]) => {
                s.presses.push("sf6-mute".into());
                let Some(app) = s.mixer["apps"]
                    .as_array_mut()
                    .and_then(|a| a.iter_mut().find(|a| a["process"] == "StreetFighter6.exe"))
                else {
                    return answer(
                        409,
                        json!({ "error": "not_running", "message": "no audio session" }),
                    );
                };
                app["muted"] = json!(!app["muted"].as_bool().unwrap_or(false));
                Self::app_changed(s);
                answer(200, json!({ "button": Self::button(s, "sf6-mute") }))
            }
            ("GET", ["audio", "mixer"]) => answer(200, s.mixer.clone()),
            ("PUT", ["audio", "master"]) => {
                if !field("volume").is_null() {
                    s.mixer["master"]["volume"] = field("volume");
                    s.mixer["master"]["muted"] = json!(false);
                }
                if !field("muted").is_null() {
                    s.mixer["master"]["muted"] = field("muted");
                }
                Self::master_changed(s);
                answer(200, s.mixer.clone())
            }
            ("PUT", ["audio", "apps", process]) => {
                let Some(app) = s.mixer["apps"]
                    .as_array_mut()
                    .and_then(|a| a.iter_mut().find(|a| a["process"] == *process))
                else {
                    return answer(404, json!({ "error": "not_found", "message": "no sound" }));
                };
                // A volume alone unmutes, as windows-link does.
                if let Some(volume) = body.and_then(|b| b.get("volume")) {
                    app["volume"] = volume.clone();
                    app["muted"] = json!(false);
                }
                if let Some(muted) = body.and_then(|b| b.get("muted")) {
                    app["muted"] = muted.clone();
                }
                Self::app_changed(s);
                answer(200, s.mixer.clone())
            }
            ("POST", ["buttons", "mute", "press"]) => {
                s.presses.push("mute".into());
                let muted = s.mixer["master"]["muted"].as_bool().unwrap_or(false);
                s.mixer["master"]["muted"] = json!(!muted);
                Self::master_changed(s);
                answer(200, json!({ "button": Self::button(s, "mute") }))
            }
            ("POST", ["buttons", id, "press"]) => {
                s.presses.push((*id).to_owned());
                if let Some((status, body)) = s.next_failure.take() {
                    return answer(status, body);
                }
                let Some(index) = s.buttons.iter().position(|b| b["id"] == *id) else {
                    return answer(404, json!({ "error": "not_found", "message": "no button" }));
                };
                let current = s.buttons[index].clone();
                let next = if current["id"] == "output" {
                    output(if current["state"]["current"] == "motu" {
                        "jbl"
                    } else {
                        "motu"
                    })
                } else {
                    volume(if current["state"]["volume"].as_f64().unwrap_or(0.) > 0.6 {
                        0.2
                    } else {
                        1.
                    })
                };
                s.buttons[index] = next.clone();
                Self::broadcast(s, &json!({ "type": "button", "button": next }));
                answer(200, json!({ "button": next }))
            }
            ("PUT", [shop, "session"]) => {
                s.sessions.push(((*shop).to_owned(), field("cookies")));
                answer(200, json!({ "kept": 1 }))
            }
            _ => answer(404, Value::Null),
        }
    }
}

impl Http for MockLink {
    fn call(&self, method: &str, url: &str, body: Option<&Value>) -> Option<Answer> {
        let path = url.strip_prefix(BASE)?;
        self.route(method, path, body)
    }
}

struct Socket;

impl Connection for Socket {
    fn stop(&self) {}
}

impl Events for MockLink {
    fn open(&self, _url: &str, sink: UnboundedSender<SocketMessage>) -> Box<dyn Connection> {
        let mut state = self.state();
        if state.down {
            let _ = sink.unbounded_send(SocketMessage::Closed);
            return Box::new(Socket);
        }
        let snapshot = json!({ "type": "snapshot", "buttons": state.buttons, "desktops": state.desktops,
            "unmatched": state.unmatched, "desktops_error": state.desktops_error });
        let _ = sink.unbounded_send(SocketMessage::Opened);
        let _ = sink.unbounded_send(SocketMessage::Text(snapshot.to_string()));
        state.sockets.push(sink);
        Box::new(Socket)
    }
}

impl SignIn for MockLink {
    fn sign_in(&self, _shop: &str, _base: &str) -> Result<Value, ErrorBody> {
        self.state().login.clone()
    }
}

/// The library state of `id` among the mock's buttons, as a map for quick checks.
pub fn ids(list: &Value) -> Vec<String> {
    list.as_array()
        .map(|l| {
            l.iter()
                .filter_map(|i| i["id"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}
