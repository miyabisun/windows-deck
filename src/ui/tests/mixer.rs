//! The mute buttons and the mixer: taps, slides of the mouse and of a finger, the keyboard and
//! each row's speaker (`e2e/mixer.spec.js`).

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, TestAppContext,
    VisualTestContext, point, px,
};
use serde_json::json;

use super::{click, has, keys, open, shows};
use crate::{
    link::mock::{MockLink, app_mute_button, mixer_button, mute_button},
    ui::theme::ROW_HEIGHT,
};

fn mock() -> MockLink {
    let mock = MockLink::default();
    {
        let mut state = mock.state();
        let master = state.mixer["master"].clone();
        state.buttons.push(mute_button(&master));
        state.buttons.push(mixer_button(&master));
    }
    mock
}

fn volume(mock: &MockLink, path: &[&str]) -> f64 {
    let state = mock.state();
    let mut value = &state.mixer;
    for part in path {
        value = match part.parse::<usize>() {
            Ok(index) => &value[index],
            Err(_) => &value[*part],
        };
    }
    value.as_f64().unwrap_or(-1.)
}

fn muted(mock: &MockLink, path: &[&str]) -> bool {
    let state = mock.state();
    let mut value = &state.mixer;
    for part in path {
        value = match part.parse::<usize>() {
            Ok(index) => &value[index],
            Err(_) => &value[*part],
        };
    }
    value.as_bool().unwrap_or(false)
}

/// Where along a slider's groove, as a share of its width.
fn along(vcx: &mut VisualTestContext, name: &str, share: f32) -> gpui::Point<gpui::Pixels> {
    let groove = super::bounds(vcx, &format!("slider-{name}")).unwrap();
    point(groove.left() + groove.size.width * share, groove.center().y)
}

#[gpui::test]
fn the_mute_button_mutes_and_unmutes_the_output(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    assert!(shows(vcx, "音量 45%"), "the volume on both");
    click(vcx, "tile-mute");
    assert!(
        shows(vcx, "ミュート解除") && shows(vcx, "ミュート中"),
        "muted"
    );
    assert!(muted(&mock, &["master", "muted"]));
    click(vcx, "tile-mute");
    assert!(!shows(vcx, "ミュート中"), "unmuted");
    assert!(!muted(&mock, &["master", "muted"]));
}

#[gpui::test]
fn the_mixer_shows_the_whole_volume_then_each_app_set_by_tap_or_slide(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-mixer");
    let whole = super::bounds(vcx, "slider-全体").unwrap();
    let discord = super::bounds(vcx, "slider-Discord").unwrap();
    let sf6 = super::bounds(vcx, "slider-StreetFighter6").unwrap();
    assert!(
        whole.top() < discord.top() && discord.top() < sf6.top(),
        "the whole first, then each app"
    );
    assert!(
        has(vcx, "value-全体-45")
            && has(vcx, "value-Discord-100")
            && has(vcx, "value-StreetFighter6-20"),
        "the values"
    );
    assert!(discord.size.height >= ROW_HEIGHT, "a finger's row");
    // A tap on the groove jumps there.
    let at = along(vcx, "StreetFighter6", 0.75);
    vcx.simulate_click(at, Modifiers::none());
    vcx.run_until_parked();
    let tapped = volume(&mock, &["apps", "1", "volume"]);
    assert!(
        tapped > 0.6 && tapped < 0.9,
        "about three quarters: {tapped}"
    );
    // A mouse slide moves it, and the value follows.
    let (from, to) = (along(vcx, "全体", 0.45), along(vcx, "全体", 0.8));
    let modifiers = Modifiers::none();
    vcx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: from,
        modifiers,
        click_count: 1,
        first_mouse: false,
    });
    for step in 1..=10 {
        #[allow(clippy::cast_precision_loss)]
        let x = from.x + (to.x - from.x) * (step as f32 / 10.);
        vcx.simulate_event(MouseMoveEvent {
            position: point(x, from.y),
            pressed_button: Some(MouseButton::Left),
            modifiers,
        });
    }
    vcx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: to,
        modifiers,
        click_count: 1,
    });
    vcx.run_until_parked();
    assert!(volume(&mock, &["master", "volume"]) > 0.7, "slid");
    assert!(!has(vcx, "value-全体-45"), "the value followed");
    // A finger slides it as well.
    let finger = along(vcx, "Discord", 0.5);
    super::slide_from(vcx, finger, -200., 0.);
    let slid = volume(&mock, &["apps", "0", "volume"]);
    assert!(slid < 0.9, "a finger slid Discord down: {slid}");
    // The keyboard works too.
    click(vcx, "slider-StreetFighter6");
    let before = volume(&mock, &["apps", "1", "volume"]);
    keys(vcx, "left");
    assert!(
        (volume(&mock, &["apps", "1", "volume"]) - (before - 0.01)).abs() < 1e-6,
        "one step down"
    );
    keys(vcx, "escape");
    assert!(!has(vcx, "slider-全体"), "Esc closes it");
}

#[gpui::test]
fn a_muted_output_says_so_and_moving_its_volume_unmutes_it(cx: &mut TestAppContext) {
    let mock = mock();
    mock.state().mixer["master"]["muted"] = json!(true);
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-mixer");
    assert!(has(vcx, "muted-全体"), "muted");
    let at = along(vcx, "全体", 0.3);
    vcx.simulate_click(at, Modifiers::none());
    vcx.run_until_parked();
    assert!(
        !muted(&mock, &["master", "muted"]),
        "moving the volume unmutes it"
    );
    assert!(!has(vcx, "muted-全体"), "no longer said");
}

#[gpui::test]
fn without_apps_playing_sound_the_mixer_says_so(cx: &mut TestAppContext) {
    let mock = mock();
    mock.state().mixer["apps"] = json!([]);
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-mixer");
    assert!(
        has(vcx, "slider-全体") && !has(vcx, "slider-Discord"),
        "the whole only"
    );
    assert!(shows(vcx, "音を出しているアプリはありません"), "said so");
}

#[gpui::test]
fn a_rows_speaker_mutes_that_app_and_keeps_its_volume(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-mixer");
    let speaker = super::bounds(vcx, "speaker-StreetFighter6").unwrap();
    assert!(
        speaker.size.width >= px(56.) && speaker.size.height >= px(56.),
        "a finger's button"
    );
    click(vcx, "speaker-StreetFighter6");
    assert!(muted(&mock, &["apps", "1", "muted"]), "that app");
    assert!(
        (volume(&mock, &["apps", "1", "volume"]) - 0.2).abs() < 1e-9,
        "its volume stays"
    );
    assert!(
        !muted(&mock, &["apps", "0", "muted"]) && !muted(&mock, &["master", "muted"]),
        "nothing else"
    );
    assert!(
        has(vcx, "value-StreetFighter6-20") && has(vcx, "muted-StreetFighter6"),
        "shown muted"
    );
    let at = along(vcx, "StreetFighter6", 0.4);
    vcx.simulate_click(at, Modifiers::none());
    vcx.run_until_parked();
    assert!(!muted(&mock, &["apps", "1", "muted"]), "a slide unmutes it");
    assert!(!has(vcx, "muted-StreetFighter6"), "shown unmuted");
}

#[gpui::test]
fn the_whole_rows_speaker_mutes_the_output(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-mixer");
    click(vcx, "speaker-全体");
    assert!(muted(&mock, &["master", "muted"]), "muted");
    assert!(
        (volume(&mock, &["master", "volume"]) - 0.45).abs() < 1e-9,
        "its volume stays"
    );
    click(vcx, "speaker-全体");
    assert!(!muted(&mock, &["master", "muted"]), "unmuted");
}

#[gpui::test]
fn an_apps_mute_button_mutes_it_as_the_mixers_speaker_does(cx: &mut TestAppContext) {
    let mock = mock();
    {
        let mut state = mock.state();
        let app = state.mixer["apps"][1].clone();
        state.buttons.push(app_mute_button(Some(&app)));
    }
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-sf6-mute");
    assert!(
        shows(vcx, "ミュート解除") && shows(vcx, "ミュート中"),
        "muted"
    );
    assert!(muted(&mock, &["apps", "1", "muted"]));
    assert!(
        (volume(&mock, &["apps", "1", "volume"]) - 0.2).abs() < 1e-9,
        "its volume stays"
    );
    // The mixer shows the same mute, and its speaker undoes it on the button too.
    click(vcx, "tile-mixer");
    assert!(has(vcx, "muted-StreetFighter6"), "the same mute");
    click(vcx, "speaker-StreetFighter6");
    keys(vcx, "escape");
    assert!(!shows(vcx, "ミュート中"), "the button follows");
}

#[gpui::test]
fn an_apps_mute_button_says_when_the_app_has_no_sound(cx: &mut TestAppContext) {
    let mock = mock();
    {
        let mut state = mock.state();
        let apps: Vec<_> = state.mixer["apps"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["process"] != "StreetFighter6.exe")
            .cloned()
            .collect();
        state.mixer["apps"] = json!(apps);
        state.buttons.push(app_mute_button(None));
    }
    let (_deck, vcx) = open(cx, &mock);
    assert!(shows(vcx, "起動していません"), "no sound");
    click(vcx, "tile-sf6-mute");
    assert!(shows(vcx, "対象のアプリが起動していません"), "why");
}
