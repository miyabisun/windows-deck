//! The virtual desktops' tabs, the + and the moon (`e2e/desktops.spec.js`).

use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::{click, has, keys, open, open_with, shows};
use crate::{
    link::mock::{MockLink, desktops},
    ui::theme::{ROW_HEIGHT, TILE_HEIGHT},
};

fn selected(vcx: &mut VisualTestContext, name: &str) -> bool {
    has(vcx, &format!("selected-{name}"))
}

fn tab_order(vcx: &mut VisualTestContext, names: &[&str]) -> bool {
    let lefts: Option<Vec<_>> = names
        .iter()
        .map(|n| super::bounds(vcx, &format!("desktop-{n}")).map(|b| b.left()))
        .collect();
    lefts.is_some_and(|l| l.windows(2).all(|w| w[0] < w[1]))
}

#[gpui::test]
fn shows_the_desktops_in_order_with_the_current_one_selected(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    assert!(
        tab_order(vcx, &["dev", "ゲーム", "ブルアカ", "アダルト"]),
        "in order"
    );
    assert!(selected(vcx, "dev"), "the current one");
    let tab = super::bounds(vcx, "desktop-dev").unwrap();
    assert_eq!(tab.size.height, TILE_HEIGHT / 2., "half a button");
}

#[gpui::test]
fn follows_switches_made_in_windows(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    mock.set_desktops(desktops(2), "changed");
    assert!(
        selected(vcx, "ブルアカ") && !selected(vcx, "dev"),
        "followed"
    );
}

#[gpui::test]
fn tapping_a_tab_asks_windows_link_to_switch(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    super::tap(vcx, "desktop-アダルト");
    assert!(selected(vcx, "アダルト"), "switched");
    assert_eq!(mock.state().switches, ["GUID-3"]);
    // The current tab does not send anything.
    click(vcx, "desktop-アダルト");
    assert_eq!(mock.state().switches, ["GUID-3"]);
}

#[gpui::test]
fn renamed_added_and_removed_desktops_update_the_tabs(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    let mut renamed = desktops(0);
    renamed[0]["name"] = json!("開発");
    mock.set_desktops(renamed.clone(), "renamed");
    assert!(
        has(vcx, "desktop-開発") && !has(vcx, "desktop-dev"),
        "renamed"
    );
    let mut added = renamed.as_array().unwrap().clone();
    added.push(json!({ "id": "GUID-4", "name": "デスクトップ 5", "index": 4, "current": false }));
    mock.set_desktops(Value::Array(added), "created");
    assert!(
        tab_order(
            vcx,
            &["開発", "ゲーム", "ブルアカ", "アダルト", "デスクトップ 5"]
        ),
        "added at the end"
    );
    let removed: Vec<Value> = renamed
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["index"] != 2)
        .cloned()
        .collect();
    mock.set_desktops(Value::Array(removed), "removed");
    assert!(
        !has(vcx, "desktop-ブルアカ") && !has(vcx, "desktop-デスクトップ 5"),
        "removed"
    );
    assert!(
        tab_order(vcx, &["開発", "ゲーム", "アダルト"]),
        "the rest stay in order"
    );
}

#[gpui::test]
fn each_tab_shows_its_own_buttons_and_the_shared_ones(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    mock.state().buttons[1]["desktop"] = json!("ゲーム");
    let (deck, vcx) = open(cx, &mock);
    assert!(
        has(vcx, "tile-output") && !has(vcx, "tile-sf6-volume"),
        "dev: the shared one only"
    );
    click(vcx, "desktop-ゲーム");
    let output = super::bounds(vcx, "tile-output").unwrap();
    let volume = super::bounds(vcx, "tile-sf6-volume").expect("ゲーム's own button");
    assert!(output.left() < volume.left(), "after the shared ones");
    let link = super::link(&deck, vcx);
    link.update(vcx, |l, cx| {
        for button in &mut l.buttons {
            button.desktop = Some("アダルト".into());
        }
        cx.notify();
    });
    assert!(
        shows(vcx, "このデスクトップに割り当てたボタンはありません。"),
        "none on this desktop"
    );
}

#[gpui::test]
fn without_virtual_desktops_there_are_no_tabs_and_every_button_shows(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    {
        let mut state = mock.state();
        state.desktops = json!([]);
        state.desktops_error = Some("virtual desktops are not enabled".into());
        state.buttons[1]["desktop"] = json!("ゲーム");
    }
    let (_deck, vcx) = open(cx, &mock);
    assert!(
        has(vcx, "tile-output") && has(vcx, "tile-sf6-volume"),
        "every button"
    );
    assert!(!has(vcx, "desktop-dev"), "no tabs");
}

#[gpui::test]
fn a_failed_switch_says_why(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    // Windows removed the desktop, but the panel has not heard yet.
    let kept: Vec<Value> = mock
        .state()
        .desktops
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["id"] != "GUID-3")
        .cloned()
        .collect();
    mock.state().desktops = Value::Array(kept);
    click(vcx, "desktop-アダルト");
    assert!(
        shows(vcx, "このデスクトップはもうありません"),
        "why, on the tab bar"
    );
    assert!(selected(vcx, "dev"), "the selection stays");
}

#[gpui::test]
fn pins_its_window_on_every_connection(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    assert_eq!(mock.state().pins, ["4723016"]);
    mock.go_down();
    vcx.run_until_parked();
    mock.come_up();
    vcx.executor()
        .advance_clock(std::time::Duration::from_secs(5));
    vcx.run_until_parked();
    assert_eq!(
        mock.state().pins,
        ["4723016", "4723016"],
        "again after reconnecting"
    );
    // Explorer restarted: windows-link reconnects to it and says so.
    mock.set_desktops(desktops(0), "reconnected");
    vcx.run_until_parked();
    assert_eq!(mock.state().pins, ["4723016", "4723016", "4723016"]);
}

#[gpui::test]
fn tabs_work_from_the_keyboard(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    // Only the current tab is a stop; the arrows move between tabs without switching.
    keys(vcx, "tab right left left");
    assert!(mock.state().switches.is_empty(), "moving does not switch");
    keys(vcx, "enter");
    assert!(
        selected(vcx, "アダルト"),
        "Enter switches to the focused tab"
    );
    keys(vcx, "tab tab tab enter");
    assert_eq!(
        mock.state().presses,
        ["output"],
        "then +, the moon and the first button"
    );
}

#[gpui::test]
fn shared_buttons_skip_the_desktops_they_except(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    mock.state().buttons[0]["except"] = json!(["dev"]);
    let (_deck, vcx) = open(cx, &mock);
    assert!(!has(vcx, "tile-output"), "not on dev");
    click(vcx, "desktop-ゲーム");
    assert!(has(vcx, "tile-output"), "on ゲーム");
}

#[gpui::test]
fn the_plus_creates_a_desktop_for_a_desktop_file_without_one(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    mock.state().unmatched = vec!["SF6".into()];
    let (deck, vcx) = open(cx, &mock);
    click(vcx, "add-desktop");
    let row = super::bounds(vcx, "create-SF6").expect("the file without a desktop");
    assert!(
        row.size.height >= ROW_HEIGHT,
        "as tall as a tab, for a finger"
    );
    click(vcx, "create-SF6");
    assert!(selected(vcx, "SF6"), "created and switched to");
    click(vcx, "add-desktop");
    assert!(
        shows(
            vcx,
            "作れるデスクトップはありません。windows-link の desktops フォルダに「名前.yaml」を置くと、ここに出ます。"
        ),
        "none left"
    );
    keys(vcx, "escape");
    assert!(
        !deck.read_with(vcx, |d, _| d.desktop_menu_open()),
        "Esc closes it"
    );
}

#[gpui::test]
fn the_moon_rests_and_only_the_wake_button_works(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (deck, vcx) = open(cx, &mock);
    click(vcx, "rest");
    assert_eq!(
        mock.state().rests,
        ["mon-panel"],
        "the panel's monitor stays lit"
    );
    assert!(has(vcx, "wake"), "the wake button");
    let link = super::link(&deck, vcx);
    assert!(link.read_with(vcx, |l, _| l.resting), "resting");
    // The panel under the cover takes no press.
    super::tap(vcx, "tile-output");
    assert!(mock.state().presses.is_empty(), "nothing pressed");
    keys(vcx, "enter");
    assert_eq!(mock.state().wakes, 1, "the wake button has the focus");
    assert!(!has(vcx, "wake"), "awake");
    click(vcx, "tile-output");
    assert_eq!(mock.state().presses, ["output"], "pressable again");
}

#[gpui::test]
fn a_wake_that_cannot_put_a_brightness_back_frees_the_panel(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    {
        let mut state = mock.state();
        state.resting = true;
        state.wake_answer = Some((
            409,
            json!({ "error": "unrestored", "message": "cannot put the brightness back on 25M2S" }),
        ));
    }
    let (_deck, vcx) = open(cx, &mock);
    // windows-link was resting before the panel opened.
    click(vcx, "wake");
    assert!(!has(vcx, "wake"), "awake");
    assert!(
        shows(
            vcx,
            "明るさを戻せなかったモニターがあります。そのモニターのボタンで戻してください（cannot put the brightness back on 25M2S）"
        ),
        "which one"
    );
    click(vcx, "tile-output");
    assert_eq!(mock.state().presses, ["output"]);
    assert_eq!(mock.state().wakes, 1);
}

#[gpui::test]
fn resting_and_waking_from_home_link_show_and_lift_the_cover(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    mock.set_resting(true);
    assert!(has(vcx, "wake"), "rested elsewhere");
    mock.set_resting(false);
    assert!(!has(vcx, "wake"), "woken elsewhere");
}

#[gpui::test]
fn resting_without_the_panels_monitor_says_why(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open_with(cx, &mock, None);
    click(vcx, "rest");
    assert!(
        shows(
            vcx,
            "deck のモニターが分からないため休止できません（設定の monitor を確かめてください）"
        ),
        "why"
    );
    assert!(mock.state().rests.is_empty(), "windows-link was not asked");
}

#[gpui::test]
fn buttons_with_an_icon_show_it(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    mock.state().buttons[1]["icon"] = json!(true);
    let (deck, vcx) = open(cx, &mock);
    let link = super::link(&deck, vcx);
    assert!(
        link.read_with(vcx, |l, _| l.buttons[1].icon),
        "windows-link has its picture"
    );
    assert!(has(vcx, "tile-sf6-volume"), "shown");
}
