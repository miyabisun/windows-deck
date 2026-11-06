//! The connection against a windows-link stand-in: scripted answers and a socket the test
//! drives.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::channel::mpsc::UnboundedSender;
use gpui::{AppContext as _, Entity, TestAppContext};
use serde_json::{Value, json};

use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub body: Option<Value>,
}

/// windows-link's answers, given in order; a request without one gets no answer.
#[derive(Default)]
pub struct FakeHttp {
    pub requests: Mutex<Vec<Request>>,
    answers: Mutex<VecDeque<Option<Answer>>>,
}

impl FakeHttp {
    pub fn answer(&self, status: u16, body: Value) {
        let body = (!body.is_null()).then_some(body);
        self.answers
            .lock()
            .unwrap()
            .push_back(Some(Answer { status, body }));
    }

    /// The next request gets no answer, as when windows-link is not running.
    pub fn fail(&self) {
        self.answers.lock().unwrap().push_back(None);
    }

    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }
}

impl Http for FakeHttp {
    fn call(&self, method: &str, url: &str, body: Option<&Value>) -> Option<Answer> {
        self.requests.lock().unwrap().push(Request {
            method: method.into(),
            url: url.into(),
            body: body.cloned(),
        });
        self.answers.lock().unwrap().pop_front().flatten()
    }
}

/// Sockets the test opens, feeds and drops.
#[derive(Default)]
pub struct FakeEvents {
    sockets: Mutex<Vec<(String, UnboundedSender<SocketMessage>)>>,
    pub stops: Arc<Mutex<usize>>,
}

struct FakeConnection(Arc<Mutex<usize>>);

impl Connection for FakeConnection {
    fn stop(&self) {
        *self.0.lock().unwrap() += 1;
    }
}

impl Events for FakeEvents {
    fn open(&self, url: &str, sink: UnboundedSender<SocketMessage>) -> Box<dyn Connection> {
        self.sockets.lock().unwrap().push((url.into(), sink));
        Box::new(FakeConnection(self.stops.clone()))
    }
}

impl FakeEvents {
    pub fn count(&self) -> usize {
        self.sockets.lock().unwrap().len()
    }

    pub fn url(&self) -> String {
        self.sockets.lock().unwrap().last().unwrap().0.clone()
    }

    fn deliver(&self, message: SocketMessage) {
        let sockets = self.sockets.lock().unwrap();
        let _ = sockets.last().unwrap().1.unbounded_send(message);
    }

    pub fn open(&self) {
        self.deliver(SocketMessage::Opened);
    }

    pub fn send(&self, message: &Value) {
        self.deliver(SocketMessage::Text(message.to_string()));
    }

    pub fn drop_socket(&self) {
        self.deliver(SocketMessage::Closed);
    }
}

/// A login window that answers what the test says.
pub struct FakeLogin(pub Mutex<Result<Value, ErrorBody>>);

impl SignIn for FakeLogin {
    fn sign_in(&self, _shop: &str, _base: &str) -> Result<Value, ErrorBody> {
        self.0.lock().unwrap().clone()
    }
}

pub struct Fake {
    pub link: Entity<Link>,
    pub http: Arc<FakeHttp>,
    pub events: Arc<FakeEvents>,
    pub login: Arc<FakeLogin>,
}

impl Fake {
    pub fn new(cx: &mut TestAppContext) -> Self {
        let http = Arc::new(FakeHttp::default());
        let events = Arc::new(FakeEvents::default());
        let login = Arc::new(FakeLogin(Mutex::new(Err(ErrorBody::named("closed")))));
        let link = cx.new(|cx| {
            let mut link = Link::new(
                "http://127.0.0.1:4730",
                http.clone(),
                events.clone(),
                login.clone(),
            );
            link.start(cx);
            link
        });
        cx.run_until_parked();
        Self {
            link,
            http,
            events,
            login,
        }
    }

    /// Connected, with `buttons` and `desktops`.
    pub fn connected(cx: &mut TestAppContext, buttons: &Value, desktops: &Value) -> Self {
        let fake = Self::new(cx);
        fake.events.open();
        fake.events
            .send(&json!({ "type": "snapshot", "buttons": buttons, "desktops": desktops }));
        cx.run_until_parked();
        fake
    }

    pub fn read<R>(&self, cx: &TestAppContext, f: impl FnOnce(&Link) -> R) -> R {
        self.link.read_with(cx, |link, _| f(link))
    }
}

fn output(current: &str) -> Value {
    json!({ "id": "output", "type": "audio.output_toggle", "label": "出力切替",
            "state": { "kind": "output", "current": current, "current_name": current, "options": [] } })
}

fn current_output(link: &Link) -> Vec<String> {
    link.buttons
        .iter()
        .map(|b| match &b.state {
            crate::model::protocol::State::Output { current, .. } => {
                current.clone().unwrap_or_default()
            }
            _ => String::new(),
        })
        .collect()
}

fn desktops(current: i64) -> Value {
    let list: Vec<Value> = ["dev", "ゲーム", "ブルアカ"]
        .iter()
        .enumerate()
        .map(|(index, name)| json!({ "id": format!("GUID-{index}"), "name": name, "index": index, "current": i64::try_from(index).unwrap() == current }))
        .collect();
    Value::Array(list)
}

fn current_desktop(link: &Link) -> String {
    link.desktops
        .iter()
        .find(|d| d.current)
        .map(|d| d.name.clone())
        .unwrap_or_default()
}

const BASE: &str = "http://127.0.0.1:4730";

#[gpui::test]
fn connects_and_takes_the_buttons_from_the_snapshot(cx: &mut TestAppContext) {
    let t = Fake::new(cx);
    assert_eq!(t.events.url(), "ws://127.0.0.1:4730/events");
    assert_eq!(t.read(cx, |l| l.status), LinkStatus::Connecting);
    t.events.open();
    t.events
        .send(&json!({ "type": "snapshot", "buttons": [output("motu")] }));
    cx.run_until_parked();
    assert_eq!(t.read(cx, |l| l.status), LinkStatus::Connected);
    assert_eq!(t.read(cx, current_output), ["motu"]);
}

#[gpui::test]
fn applies_button_events_and_ignores_others(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([output("motu")]), &json!([]));
    t.events
        .send(&json!({ "type": "button", "button": output("jbl") }));
    t.events.send(&json!({ "type": "weather" }));
    cx.run_until_parked();
    assert_eq!(t.read(cx, current_output), ["jbl"]);
}

#[gpui::test]
fn presses_a_button_showing_it_pending_and_takes_the_answer(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([output("motu")]), &json!([]));
    t.link.update(cx, |l, cx| l.press("output", cx)).detach();
    assert!(t.read(cx, |l| l.pending.contains("output")), "pending");
    // A second press meanwhile asks nothing more.
    t.link.update(cx, |l, cx| l.press("output", cx)).detach();
    t.http.answer(200, json!({ "button": output("jbl") }));
    cx.run_until_parked();
    let requests = t.http.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        (requests[0].method.as_str(), requests[0].url.as_str()),
        ("POST", "http://127.0.0.1:4730/buttons/output/press")
    );
    assert!(
        !t.read(cx, |l| l.pending.contains("output")),
        "no longer pending"
    );
    assert_eq!(t.read(cx, current_output), ["jbl"]);
}

#[gpui::test]
fn keeps_a_failed_press_until_the_next(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([output("motu")]), &json!([]));
    t.http.answer(
        409,
        json!({ "error": "device_unavailable", "message": "jbl is not connected" }),
    );
    t.link.update(cx, |l, cx| l.press("output", cx)).detach();
    cx.run_until_parked();
    assert_eq!(
        t.read(cx, |l| l.failures.get("output").cloned()).as_deref(),
        Some("切替先のデバイスがつながっていません")
    );
    assert_eq!(t.read(cx, current_output), ["motu"]);
    t.link.update(cx, |l, cx| l.press("output", cx)).detach();
    assert_eq!(
        t.read(cx, |l| l.failures.get("output").cloned()),
        None,
        "the next press clears it"
    );
    t.http.fail();
    cx.run_until_parked();
    assert_eq!(
        t.read(cx, |l| l.failures.get("output").cloned()).as_deref(),
        Some("windows-link に届きませんでした")
    );
}

#[gpui::test]
fn reconnects_with_a_growing_delay_and_does_not_press_meanwhile(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([output("motu")]), &json!([]));
    t.events.drop_socket();
    cx.run_until_parked();
    assert_eq!(t.read(cx, |l| l.status), LinkStatus::Disconnected);
    assert_eq!(t.read(cx, |l| l.buttons.len()), 1, "the last buttons stay");
    t.link.update(cx, |l, cx| l.press("output", cx)).detach();
    cx.run_until_parked();
    assert!(t.http.requests().is_empty(), "no press while disconnected");

    for delay in [1000, 2000, 4000, 5000, 5000] {
        let before = t.events.count();
        cx.executor()
            .advance_clock(Duration::from_millis(delay - 1));
        cx.run_until_parked();
        assert_eq!(t.events.count(), before, "still waiting before {delay} ms");
        cx.executor().advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        assert_eq!(t.events.count(), before + 1, "reconnected after {delay} ms");
        t.events.drop_socket();
        cx.run_until_parked();
    }
    cx.executor().advance_clock(MAX_RETRY);
    cx.run_until_parked();
    t.events.open();
    t.events
        .send(&json!({ "type": "snapshot", "buttons": [output("jbl")] }));
    cx.run_until_parked();
    assert_eq!(t.read(cx, |l| l.status), LinkStatus::Connected);
    t.events.drop_socket();
    cx.run_until_parked();
    let before = t.events.count();
    cx.executor().advance_clock(FIRST_RETRY);
    cx.run_until_parked();
    assert_eq!(
        t.events.count(),
        before + 1,
        "an open connection starts the delays over"
    );
}

#[gpui::test]
fn stops_reconnecting_once_stopped(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    t.link.update(cx, |l, _| l.stop());
    assert_eq!(*t.events.stops.lock().unwrap(), 1, "the socket is closed");
    t.events.drop_socket();
    cx.executor().advance_clock(MAX_RETRY * 2);
    cx.run_until_parked();
    assert_eq!(t.events.count(), 1, "no new connection");
}

#[gpui::test]
fn follows_the_desktops(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &desktops(0));
    assert_eq!(
        t.read(cx, |l| l
            .desktops
            .iter()
            .map(|d| d.name.clone())
            .collect::<Vec<_>>()),
        ["dev", "ゲーム", "ブルアカ"]
    );
    assert_eq!(t.read(cx, |l| l.pin_round), 1);
    t.events.send(
        &json!({ "type": "desktops", "reason": "changed", "desktops": desktops(2), "error": null }),
    );
    cx.run_until_parked();
    assert_eq!(t.read(cx, current_desktop), "ブルアカ");
}

#[gpui::test]
fn has_no_desktops_when_windows_link_cannot_reach_them(cx: &mut TestAppContext) {
    let t = Fake::new(cx);
    t.events.send(&json!({ "type": "snapshot", "buttons": [], "desktops": [], "desktops_error": "virtual desktops are not enabled" }));
    cx.run_until_parked();
    assert!(t.read(cx, |l| l.desktops.is_empty()), "no desktops");
    assert_eq!(
        t.read(cx, |l| l.desktops_error.clone()).as_deref(),
        Some("virtual desktops are not enabled")
    );
}

#[gpui::test]
fn pins_again_after_reconnecting_or_an_explorer_restart(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    t.events.drop_socket();
    cx.run_until_parked();
    cx.executor().advance_clock(FIRST_RETRY);
    cx.run_until_parked();
    t.events.open();
    t.events
        .send(&json!({ "type": "snapshot", "buttons": [], "desktops": [] }));
    cx.run_until_parked();
    assert_eq!(t.read(cx, |l| l.pin_round), 2);
    t.events.send(
        &json!({ "type": "desktops", "reason": "changed", "desktops": desktops(1), "error": null }),
    );
    cx.run_until_parked();
    assert_eq!(t.read(cx, |l| l.pin_round), 2);
    t.events.send(&json!({ "type": "desktops", "reason": "reconnected", "desktops": desktops(1), "error": null }));
    cx.run_until_parked();
    assert_eq!(t.read(cx, |l| l.pin_round), 3);
}

#[gpui::test]
fn switches_desktops_one_at_a_time(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &desktops(0));
    t.link
        .update(cx, |l, cx| l.switch_desktop("GUID-1", cx))
        .detach();
    assert_eq!(
        t.read(cx, |l| l.switching.clone()).as_deref(),
        Some("GUID-1")
    );
    t.link
        .update(cx, |l, cx| l.switch_desktop("GUID-2", cx))
        .detach();
    t.http
        .answer(200, json!({ "desktops": desktops(1), "error": null }));
    cx.run_until_parked();
    let requests = t.http.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        (requests[0].method.as_str(), requests[0].url.as_str()),
        ("POST", "http://127.0.0.1:4730/desktops/GUID-1/switch")
    );
    assert_eq!(t.read(cx, |l| l.switching.clone()), None);
    assert_eq!(t.read(cx, current_desktop), "ゲーム");
}

#[gpui::test]
fn says_why_a_switch_failed_until_the_desktops_change(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &desktops(0));
    t.http.answer(
        404,
        json!({ "error": "not_found", "message": "no desktop with this id" }),
    );
    t.link
        .update(cx, |l, cx| l.switch_desktop("GUID-9", cx))
        .detach();
    cx.run_until_parked();
    assert_eq!(
        t.read(cx, |l| l.desktop_failure.clone()).as_deref(),
        Some("このデスクトップはもうありません")
    );
    t.events.send(
        &json!({ "type": "desktops", "reason": "removed", "desktops": desktops(0), "error": null }),
    );
    cx.run_until_parked();
    assert_eq!(t.read(cx, |l| l.desktop_failure.clone()), None);
    t.http.fail();
    t.link
        .update(cx, |l, cx| l.switch_desktop("GUID-1", cx))
        .detach();
    cx.run_until_parked();
    assert_eq!(
        t.read(cx, |l| l.desktop_failure.clone()).as_deref(),
        Some("windows-link に届きませんでした")
    );
}

#[gpui::test]
async fn pins_a_window_and_says_whether_it_worked(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &desktops(0));
    t.http.answer(200, json!({ "pinned": true }));
    let pinned = t.link.update(cx, |l, cx| l.pin(4_723_016, cx));
    assert!(pinned.await, "pinned");
    assert_eq!(
        t.http.requests()[0].url,
        "http://127.0.0.1:4730/windows/4723016/pin"
    );
    t.http.answer(
        503,
        json!({ "error": "desktops", "message": "not enabled" }),
    );
    let pinned = t.link.update(cx, |l, cx| l.pin(4_723_016, cx));
    assert!(!pinned.await, "not pinned");
}

#[gpui::test]
fn creates_a_desktop_for_a_desktop_file(cx: &mut TestAppContext) {
    let t = Fake::new(cx);
    t.events.open();
    t.events.send(&json!({ "type": "snapshot", "buttons": [], "desktops": desktops(0), "unmatched": ["SF6"] }));
    cx.run_until_parked();
    assert_eq!(t.read(cx, |l| l.unmatched.clone()), ["SF6"]);
    let mut list = desktops(-1).as_array().unwrap().clone();
    list.push(json!({ "id": "GUID-3", "name": "SF6", "index": 3, "current": true }));
    t.http.answer(
        201,
        json!({ "desktops": list, "unmatched": [], "error": null }),
    );
    t.link
        .update(cx, |l, cx| l.create_desktop("SF6", cx))
        .detach();
    cx.run_until_parked();
    let request = &t.http.requests()[0];
    assert_eq!(
        (request.method.as_str(), request.url.as_str()),
        ("POST", "http://127.0.0.1:4730/desktops")
    );
    assert_eq!(request.body, Some(json!({ "name": "SF6" })));
    assert_eq!(t.read(cx, current_desktop), "SF6");
    assert!(t.read(cx, |l| l.unmatched.is_empty()), "none left");

    t.http.answer(
        409,
        json!({ "error": "exists", "message": "a desktop with this name already exists" }),
    );
    t.link
        .update(cx, |l, cx| l.create_desktop("SF6", cx))
        .detach();
    cx.run_until_parked();
    assert_eq!(
        t.read(cx, |l| l.desktop_failure.clone()).as_deref(),
        Some("「SF6」はもうあります")
    );
}

#[gpui::test]
async fn mutes_an_app_by_itself_and_answers_with_the_mixer(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    let mixer = json!({ "master": { "volume": 0.5, "muted": false }, "apps": [] });
    t.http.answer(200, mixer.clone());
    let changed = t.link.update(cx, |l, cx| {
        l.set_app(
            "BlueArchive.exe",
            VolumeChange {
                muted: Some(true),
                ..VolumeChange::default()
            },
            cx,
        )
    });
    let changed = changed.await;
    assert_eq!(changed, Ok(serde_json::from_value(mixer).unwrap()));
    let request = &t.http.requests()[0];
    assert_eq!(
        (request.method.as_str(), request.url.as_str()),
        ("PUT", "http://127.0.0.1:4730/audio/apps/BlueArchive.exe")
    );
    assert_eq!(request.body, Some(json!({ "muted": true })));

    t.http
        .answer(404, json!({ "error": "not_found", "message": "no sound" }));
    let changed = t.link.update(cx, |l, cx| {
        l.set_app(
            "BlueArchive.exe",
            VolumeChange {
                volume: Some(0.3),
                ..VolumeChange::default()
            },
            cx,
        )
    });
    assert_eq!(
        changed.await,
        Err("このアプリは今は音を出していません（ミキサーを開き直してください）".into())
    );
    assert_eq!(t.http.requests()[1].body, Some(json!({ "volume": 0.3 })));
}

fn monitors() -> Value {
    json!([
        { "id": "mon-main", "name": "25M2S", "gdi_name": "DISPLAY1", "primary": true, "touch": false },
        { "id": "mon-panel", "name": "T10FA", "gdi_name": "DISPLAY5", "primary": false, "touch": true }
    ])
}

#[gpui::test]
fn reads_whether_windows_link_is_resting(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    t.http.answer(200, json!({ "resting": true }));
    t.link.update(cx, |l, cx| l.sync_rest(cx)).detach();
    cx.run_until_parked();
    assert_eq!(t.http.requests()[0].url, "http://127.0.0.1:4730/power");
    assert!(t.read(cx, |l| l.resting), "resting");
}

#[gpui::test]
fn rests_keeping_the_configured_monitor_lit(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    t.http.answer(200, monitors());
    t.http.answer(200, json!({ "resting": true }));
    t.link
        .update(cx, |l, cx| l.rest(Some("T10FA"), cx))
        .detach();
    assert_eq!(t.read(cx, |l| l.rest_pending), Some(RestStep::Rest));
    cx.run_until_parked();
    let requests = t.http.requests();
    assert_eq!(requests[0].url, "http://127.0.0.1:4730/touch-monitors");
    assert_eq!(
        (requests[1].method.as_str(), requests[1].url.as_str()),
        ("POST", "http://127.0.0.1:4730/power/rest")
    );
    assert_eq!(requests[1].body, Some(json!({ "keep": "mon-panel" })));
    assert!(t.read(cx, |l| l.resting), "resting");
    assert_eq!(t.read(cx, |l| l.rest_failure.clone()), None);
}

#[gpui::test]
fn does_not_rest_without_the_panels_monitor(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    t.http.answer(200, monitors());
    t.link.update(cx, |l, cx| l.rest(None, cx)).detach();
    cx.run_until_parked();
    assert_eq!(t.http.requests().len(), 1);
    assert!(!t.read(cx, |l| l.resting), "awake");
    assert!(
        t.read(cx, |l| l.rest_failure.clone())
            .unwrap()
            .contains("deck のモニターが分からない"),
        "why"
    );

    t.http.answer(200, monitors());
    t.http.answer(
        404,
        json!({ "error": "not_found", "message": "no connected monitor with this id" }),
    );
    t.link
        .update(cx, |l, cx| l.rest(Some("mon-panel"), cx))
        .detach();
    cx.run_until_parked();
    assert!(!t.read(cx, |l| l.resting), "awake");
    assert_eq!(
        t.read(cx, |l| l.rest_failure.clone()).as_deref(),
        Some("deck のモニターが windows-link に見つかりません")
    );
}

#[gpui::test]
fn wakes_and_says_which_brightness_stayed_low(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    t.link.update(cx, |l, _| l.resting = true);
    t.http.answer(
        409,
        json!({ "error": "unrestored", "message": "cannot put the brightness back on 25M2S" }),
    );
    t.link.update(cx, super::Link::wake).detach();
    cx.run_until_parked();
    assert_eq!(
        (
            t.http.requests()[0].method.as_str(),
            t.http.requests()[0].url.as_str()
        ),
        ("POST", "http://127.0.0.1:4730/power/wake")
    );
    // windows-link is awake, having tried again itself; the panel is too.
    assert!(!t.read(cx, |l| l.resting), "awake");
    assert!(
        t.read(cx, |l| l.rest_failure.clone())
            .unwrap()
            .contains("25M2S"),
        "which monitor"
    );
    // windows-link then says it is awake; the reason stays.
    t.events.send(&json!({ "type": "power", "resting": false }));
    cx.run_until_parked();
    assert!(
        t.read(cx, |l| l.rest_failure.clone())
            .unwrap()
            .contains("25M2S"),
        "kept"
    );
}

#[gpui::test]
fn follows_resting_elsewhere_dropping_a_stale_reason(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    t.events.send(&json!({ "type": "power", "resting": true }));
    cx.run_until_parked();
    assert!(t.read(cx, |l| l.resting), "resting");
    t.link.update(cx, |l, _| {
        l.rest_failure = Some("windows-link に届きませんでした".into());
    });
    t.events.send(&json!({ "type": "power", "resting": false }));
    cx.run_until_parked();
    assert!(!t.read(cx, |l| l.resting), "awake");
    assert_eq!(t.read(cx, |l| l.rest_failure.clone()), None);
}

#[gpui::test]
fn ignores_a_second_wake_under_way(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    t.link.update(cx, |l, _| l.resting = true);
    t.http.fail();
    t.link.update(cx, super::Link::wake).detach();
    t.link.update(cx, super::Link::wake).detach();
    cx.run_until_parked();
    assert_eq!(t.http.requests().len(), 1);
    assert_eq!(
        t.read(cx, |l| l.rest_failure.clone()).as_deref(),
        Some("windows-link に届きませんでした")
    );
    assert!(t.read(cx, |l| l.resting), "still resting");
}

fn library_button(pins: &Value) -> Value {
    json!({ "id": "games", "type": "steam.library", "label": "Steam", "state": { "kind": "library", "pins": pins } })
}

#[gpui::test]
async fn hands_a_shops_login_cookies_to_windows_link(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([library_button(&json!([]))]), &json!([]));
    let cookies = json!([{ "name": "login_secure_id", "value": "v", "domain": "dmm.co.jp" }]);
    *t.login.0.lock().unwrap() = Ok(cookies.clone());
    t.http.answer(200, json!({ "kept": 1 }));
    let done = t.link.update(cx, |l, cx| l.sign_in("fanza", cx));
    assert_eq!(done.await, None);
    let request = &t.http.requests()[0];
    assert_eq!(
        (request.method.as_str(), request.url.as_str()),
        ("PUT", "http://127.0.0.1:4730/fanza/session")
    );
    assert_eq!(request.body, Some(json!({ "cookies": cookies })));

    t.http
        .answer(400, json!({ "error": "invalid_session", "message": "x" }));
    let refused = t.link.update(cx, |l, cx| l.sign_in("fanza", cx));
    assert_eq!(
        refused.await.as_deref(),
        Some("ログインを確認できませんでした。もう一度ログインしてください")
    );

    // Closed before signing in: nothing goes to windows-link.
    *t.login.0.lock().unwrap() = Err(ErrorBody::named("closed"));
    let closed = t.link.update(cx, |l, cx| l.sign_in("fanza", cx));
    assert_eq!(
        closed.await.as_deref(),
        Some("ログインせずにログイン画面が閉じられました")
    );
    assert_eq!(t.http.requests().len(), 2);
}

#[gpui::test]
async fn reads_a_library(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([library_button(&json!([]))]), &json!([]));
    t.http.answer(
        200,
        json!({ "items": [], "labels": [], "hide": [], "partial": null }),
    );
    let read = t.link.update(cx, |l, cx| l.library("games", cx));
    assert_eq!(read.await, Ok(Library::default()));
    assert_eq!(
        t.http.requests()[0].url,
        "http://127.0.0.1:4730/buttons/games/library"
    );
    t.http.fail();
    let failed = t.link.update(cx, |l, cx| l.library("games", cx));
    assert_eq!(failed.await, Err("windows-link に届きませんでした".into()));
}

#[gpui::test]
async fn starts_a_game_showing_it_pending(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([library_button(&json!([]))]), &json!([]));
    let key = item_key("games", "1364780");
    t.http.answer(204, Value::Null);
    let started = t
        .link
        .update(cx, |l, cx| l.start_item("games", "1364780", cx));
    assert!(t.read(cx, |l| l.pending.contains(&key)), "pending");
    assert_eq!(started.await, Started::Started);
    assert_eq!(
        (
            t.http.requests()[0].method.as_str(),
            t.http.requests()[0].url.as_str()
        ),
        (
            "POST",
            "http://127.0.0.1:4730/buttons/games/library/1364780/start"
        )
    );
    assert!(!t.read(cx, |l| l.pending.contains(&key)), "done");

    t.http
        .answer(500, json!({ "error": "launch", "message": "denied" }));
    let started = t
        .link
        .update(cx, |l, cx| l.start_item("games", "1364780", cx));
    assert_eq!(started.await, Started::Failed);
    assert_eq!(
        t.read(cx, |l| l.failures.get(&key).cloned()).as_deref(),
        Some("起動できませんでした: denied")
    );

    // A game with several programs asks which one instead of failing.
    t.http.answer(
        409,
        json!({ "error": "choose_program", "message": "choose" }),
    );
    let started = t
        .link
        .update(cx, |l, cx| l.start_item("games", "1364780", cx));
    assert_eq!(started.await, Started::Choose);
    assert_eq!(t.read(cx, |l| l.failures.get(&key).cloned()), None);
}

#[gpui::test]
async fn updates_a_library_showing_its_button_pending(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([library_button(&json!([]))]), &json!([]));
    t.http.answer(200, json!({ "updates": 1, "installs": 0 }));
    let updated = t.link.update(cx, |l, cx| l.update_library("games", cx));
    assert!(t.read(cx, |l| l.pending.contains("games")), "pending");
    let again = t.link.update(cx, |l, cx| l.update_library("games", cx));
    assert_eq!(again.await, None);
    assert_eq!(
        updated.await.as_deref(),
        Some("アップデート 1 件を始めました")
    );
    assert_eq!(t.http.requests().len(), 1);
    assert_eq!(
        t.http.requests()[0].url,
        "http://127.0.0.1:4730/buttons/games/library/update"
    );

    t.http.answer(
        409,
        json!({ "error": "update_unavailable", "message": "Steam is not running" }),
    );
    let updated = t.link.update(cx, |l, cx| l.update_library("games", cx));
    assert_eq!(updated.await, None);
    assert_eq!(
        t.read(cx, |l| l.failures.get("games").cloned()).as_deref(),
        Some("Steam が起動していないため、ライブラリを更新できません")
    );

    t.http.answer(202, json!({ "round": true }));
    let updated = t.link.update(cx, |l, cx| l.update_library("games", cx));
    assert_eq!(
        t.read(cx, |l| l.failures.get("games").cloned()),
        None,
        "the next update starts clean"
    );
    assert_eq!(
        updated.await.as_deref(),
        Some("ダウンロードと更新を始めました（進み具合はライブラリに出ます）")
    );
}

#[gpui::test]
async fn reads_programs_and_remembers_the_chosen_one(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    t.http.answer(
        200,
        json!({ "candidates": ["a.exe", "b.exe"], "chosen": null }),
    );
    let read = t.link.update(cx, |l, cx| l.programs("games", "abc", cx));
    assert_eq!(
        read.await,
        Ok(Programs {
            candidates: vec!["a.exe".into(), "b.exe".into()],
            chosen: None
        })
    );
    assert_eq!(
        t.http.requests()[0].url,
        format!("{BASE}/buttons/games/library/abc/programs")
    );
    t.http.answer(204, Value::Null);
    let chosen = t
        .link
        .update(cx, |l, cx| l.choose_program("games", "abc", "b.exe", cx));
    assert_eq!(chosen.await, None);
    let request = &t.http.requests()[1];
    assert_eq!(
        (request.method.as_str(), request.url.as_str()),
        (
            "PUT",
            "http://127.0.0.1:4730/buttons/games/library/abc/program"
        )
    );
    assert_eq!(request.body, Some(json!({ "program": "b.exe" })));
}

#[gpui::test]
async fn pins_and_unpins_a_game(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([library_button(&json!([]))]), &json!([]));
    let sf6 = json!({ "id": "1364780", "name": "Street Fighter 6" });
    t.http
        .answer(200, json!({ "button": library_button(&json!([sf6])) }));
    let pinned = t
        .link
        .update(cx, |l, cx| l.set_pinned("games", "1364780", true, cx));
    assert_eq!(pinned.await, None);
    assert_eq!(
        (
            t.http.requests()[0].method.as_str(),
            t.http.requests()[0].url.as_str()
        ),
        ("PUT", "http://127.0.0.1:4730/buttons/games/pins/1364780")
    );
    let pins = t.read(cx, |l| match &l.buttons[0].state {
        crate::model::protocol::State::Library(library) => library.pins.len(),
        _ => 0,
    });
    assert_eq!(pins, 1);
    t.http.answer(
        404,
        json!({ "error": "not_found", "message": "no such game" }),
    );
    let unpinned = t
        .link
        .update(cx, |l, cx| l.set_pinned("games", "1364780", false, cx));
    assert_eq!(
        unpinned.await.as_deref(),
        Some("ライブラリにこのゲームがありません（一覧を開き直してください）")
    );
    assert_eq!(t.http.requests()[1].method, "DELETE");
}

#[gpui::test]
async fn chooses_the_cursor_colour(cx: &mut TestAppContext) {
    let cursor = |color: &str| json!({ "id": "cursor", "type": "cursor.highlight", "label": "マウスを目立たせる", "state": { "kind": "highlight", "on": false, "color": color } });
    let t = Fake::connected(cx, &json!([cursor("blue")]), &json!([]));
    t.http.answer(200, json!({ "button": cursor("red") }));
    let chosen = t.link.update(cx, |l, cx| l.set_color("cursor", "red", cx));
    assert_eq!(chosen.await, None);
    let request = &t.http.requests()[0];
    assert_eq!(
        (request.method.as_str(), request.url.as_str()),
        ("PUT", "http://127.0.0.1:4730/buttons/cursor/color")
    );
    assert_eq!(request.body, Some(json!({ "color": "red" })));
    let color = t.read(cx, |l| match &l.buttons[0].state {
        crate::model::protocol::State::Highlight { color, .. } => color.clone(),
        _ => None,
    });
    assert_eq!(color.as_deref(), Some("red"));
    t.http.answer(
        404,
        json!({ "error": "not_found", "message": "unknown button" }),
    );
    let failed = t
        .link
        .update(cx, |l, cx| l.set_color("cursor", "green", cx));
    let why = "windows-link にこのボタンがありません（設定が変わった可能性があります）";
    assert_eq!(failed.await.as_deref(), Some(why));
    assert_eq!(
        t.read(cx, |l| l.failures.get("cursor").cloned()).as_deref(),
        Some(why)
    );
}

#[gpui::test]
async fn opens_a_games_folder_and_shop_page(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    t.http.answer(204, Value::Null);
    let opened = t
        .link
        .update(cx, |l, cx| l.open_folder("games", "1364780", cx));
    assert_eq!(opened.await, None);
    assert_eq!(
        t.http.requests()[0].url,
        format!("{BASE}/buttons/games/library/1364780/folder")
    );
    t.http.answer(
        404,
        json!({ "error": "not_found", "message": "the game is not installed" }),
    );
    let missing = t.link.update(cx, |l, cx| l.open_folder("games", "2", cx));
    assert_eq!(
        missing.await.as_deref(),
        Some("このゲームはインストールされていません")
    );

    t.http.answer(204, Value::Null);
    let page = t
        .link
        .update(cx, |l, cx| l.open_page("dlsite", "RJ01707484", cx));
    assert_eq!(page.await, None);
    assert_eq!(
        (
            t.http.requests()[2].method.as_str(),
            t.http.requests()[2].url.as_str()
        ),
        (
            "POST",
            "http://127.0.0.1:4730/buttons/dlsite/library/RJ01707484/page"
        )
    );
    t.http.answer(
        404,
        json!({ "error": "not_found", "message": "the game has no page in its shop" }),
    );
    let unknown = t
        .link
        .update(cx, |l, cx| l.open_page("dlsite", "0a1b2c3d", cx));
    assert_eq!(
        unknown.await.as_deref(),
        Some("このゲームの販売ページは分かりません")
    );
}

#[gpui::test]
async fn reads_how_a_round_goes(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    t.http.answer(
        200,
        json!({ "doing": null, "started": 1, "finished": 2, "purchases": null, "jobs": [] }),
    );
    let read = t.link.update(cx, |l, cx| l.library_round("dlsite", cx));
    assert_eq!(
        read.await,
        Ok(Round {
            started: Some(1),
            finished: Some(2),
            ..Round::default()
        })
    );
    assert_eq!(
        (
            t.http.requests()[0].method.as_str(),
            t.http.requests()[0].url.as_str()
        ),
        ("GET", "http://127.0.0.1:4730/buttons/dlsite/library/update")
    );
    t.http.fail();
    let down = t.link.update(cx, |l, cx| l.library_round("dlsite", cx));
    assert_eq!(down.await, Err("windows-link に届きませんでした".into()));
}

#[gpui::test]
async fn creates_renames_and_deletes_labels(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    t.http.answer(
        201,
        json!({ "label": { "id": "uc-9", "name": "RPG", "editable": true } }),
    );
    let created = t
        .link
        .update(cx, |l, cx| l.create_label("games", "RPG", cx));
    assert_eq!(
        created.await,
        Ok(Label {
            id: "uc-9".into(),
            name: "RPG".into(),
            editable: true
        })
    );
    assert_eq!(t.http.requests()[0].body, Some(json!({ "name": "RPG" })));
    assert_eq!(
        t.http.requests()[0].url,
        format!("{BASE}/buttons/games/labels")
    );
    t.http.answer(204, Value::Null);
    let renamed = t
        .link
        .update(cx, |l, cx| l.rename_label("games", "uc-9", "JRPG", cx));
    assert_eq!(renamed.await, None);
    let request = &t.http.requests()[1];
    assert_eq!(
        (request.method.as_str(), request.url.as_str()),
        ("PATCH", "http://127.0.0.1:4730/buttons/games/labels/uc-9")
    );
    assert_eq!(request.body, Some(json!({ "name": "JRPG" })));
    t.http.answer(
        409,
        json!({ "error": "labels_unavailable", "message": "Steam が起動していません" }),
    );
    let deleted = t
        .link
        .update(cx, |l, cx| l.delete_label("games", "uc-9", cx));
    assert_eq!(
        deleted.await.as_deref(),
        Some("ラベルを変更できません: Steam が起動していません")
    );
    assert_eq!(t.http.requests()[2].method, "DELETE");
}

#[gpui::test]
async fn puts_a_game_in_a_label_and_takes_it_out(cx: &mut TestAppContext) {
    let t = Fake::connected(cx, &json!([]), &json!([]));
    t.http.answer(204, Value::Null);
    let added = t.link.update(cx, |l, cx| {
        l.set_label("games", "hidden", "1364780", true, cx)
    });
    assert_eq!(added.await, None);
    assert_eq!(
        (
            t.http.requests()[0].method.as_str(),
            t.http.requests()[0].url.as_str()
        ),
        (
            "PUT",
            "http://127.0.0.1:4730/buttons/games/labels/hidden/items/1364780"
        )
    );
    t.http.fail();
    let removed = t.link.update(cx, |l, cx| {
        l.set_label("games", "hidden", "1364780", false, cx)
    });
    assert_eq!(
        removed.await.as_deref(),
        Some("windows-link に届きませんでした")
    );
    assert_eq!(t.http.requests()[1].method, "DELETE");
}
