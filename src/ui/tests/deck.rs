//! The buttons: their order and state, presses, failures, the connection, the keyboard and the
//! touch screen (`e2e/deck.spec.js`).

use gpui::{TestAppContext, px, size};
use serde_json::json;

use super::{click, has, keys, long_press, open, right_click, shows, tap};
use crate::{
    link::mock::{MockLink, highlight_button, output, volume},
    ui::{
        theme::{SP_3, SP_4, TILE_HEIGHT, TILE_WIDTH},
        tile::columns,
    },
};

#[gpui::test]
fn shows_the_buttons_in_order_with_their_state(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    let output = super::bounds(vcx, "tile-output").expect("output");
    let volume = super::bounds(vcx, "tile-sf6-volume").expect("volume");
    assert!(output.left() < volume.left(), "in windows-link's order");
    assert!(
        shows(vcx, "出力切替") && shows(vcx, "MOTU (MOTU M Series)"),
        "the output and its device"
    );
    assert!(shows(vcx, "スト6 音量") && shows(vcx, "100%"), "the volume");
    assert!(
        output.size.height >= TILE_HEIGHT,
        "at least a button's height"
    );
}

#[gpui::test]
fn a_press_shows_progress_then_the_new_state(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (deck, vcx) = open(cx, &mock);
    let link = super::link(&deck, vcx);
    // While a press is under way the tile is busy, and a second press asks nothing more.
    link.update(vcx, |l, cx| {
        l.pending.insert("output".into());
        cx.notify();
    });
    assert!(has(vcx, "busy-output"), "busy");
    click(vcx, "tile-output");
    assert!(mock.state().presses.is_empty(), "ignored while busy");
    link.update(vcx, |l, cx| {
        l.pending.clear();
        cx.notify();
    });
    click(vcx, "tile-output");
    assert!(shows(vcx, "BTイヤホン (JBL Tour Pro 3)"), "the new device");
    assert!(!has(vcx, "busy-output"), "no longer busy");
    assert_eq!(mock.state().presses, ["output"]);
    click(vcx, "tile-sf6-volume");
    assert!(shows(vcx, "20%"), "the new volume");
}

#[gpui::test]
fn a_failed_press_says_why_until_pressed_again(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    mock.state().next_failure = Some((
        409,
        json!({ "error": "device_unavailable", "message": "jbl is not connected" }),
    ));
    click(vcx, "tile-output");
    assert!(shows(vcx, "切替先のデバイスがつながっていません"), "why");
    assert!(
        has(vcx, "failed-output") && !has(vcx, "failed-sf6-volume"),
        "only that button failed"
    );
    assert!(shows(vcx, "MOTU (MOTU M Series)"), "the state stays");
    click(vcx, "tile-output");
    assert!(
        !shows(vcx, "切替先のデバイスがつながっていません"),
        "cleared by the next press"
    );
    assert!(shows(vcx, "BTイヤホン (JBL Tour Pro 3)"), "pressed");
}

#[gpui::test]
fn changes_made_outside_the_panel_show_up(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    mock.change(volume(0.35));
    assert!(shows(vcx, "35%"), "followed");
    let mut stopped = volume(0.);
    stopped["state"] =
        json!({ "kind": "volume", "running": false, "volume": null, "levels": [0.2, 1] });
    mock.change(stopped);
    assert!(shows(vcx, "起動していません"), "not running");
}

#[gpui::test]
fn losing_windows_link_disables_the_buttons_until_it_is_back(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    mock.go_down();
    vcx.run_until_parked();
    assert!(
        shows(
            vcx,
            "windows-link（http://127.0.0.1:4730）に接続できません。再接続しています…"
        ),
        "the notice"
    );
    assert!(has(vcx, "disabled-output"), "the buttons cannot be pressed");
    super::tap(vcx, "tile-output");
    assert!(mock.state().presses.is_empty(), "nothing pressed");
    mock.come_up();
    vcx.executor()
        .advance_clock(std::time::Duration::from_secs(5));
    vcx.run_until_parked();
    assert!(
        !shows(
            vcx,
            "windows-link（http://127.0.0.1:4730）に接続できません。再接続しています…"
        ),
        "back"
    );
    click(vcx, "tile-output");
    assert!(shows(vcx, "BTイヤホン (JBL Tour Pro 3)"), "pressed again");
}

#[gpui::test]
fn says_how_to_add_buttons_when_there_are_none(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    mock.state().buttons.clear();
    let (_deck, vcx) = open(cx, &mock);
    assert!(shows(vcx, "ボタンがありません。"), "none");
    assert!(
        shows(
            vcx,
            "windows-link の設定ファイル（%LOCALAPPDATA%\\windows-link\\config.yaml）にボタンを追加し、windows-link を再起動してください。"
        ),
        "where to add them"
    );
}

#[gpui::test]
fn works_from_the_keyboard(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    mock.state().desktops = json!([]);
    let (deck, vcx) = open(cx, &mock);
    // The tab bar comes first: create a desktop, then rest, then the buttons.
    keys(vcx, "tab enter");
    assert!(
        deck.read_with(vcx, |d, _| d.desktop_menu_open()),
        "+ opened its menu"
    );
    keys(vcx, "escape");
    assert!(
        !deck.read_with(vcx, |d, _| d.desktop_menu_open()),
        "Esc closed it"
    );
    keys(vcx, "tab tab enter");
    assert_eq!(
        mock.state().presses,
        ["output"],
        "the third stop is the first button"
    );
    assert!(
        shows(vcx, "BTイヤホン (JBL Tour Pro 3)"),
        "pressed with Enter"
    );
}

#[gpui::test]
fn the_grid_has_seven_columns_at_1920_and_five_at_1280(cx: &mut TestAppContext) {
    for (width, height, count) in [(1920., 1080., 7), (1280., 800., 5)] {
        assert_eq!(
            columns(px(width) - SP_4 * 2., TILE_WIDTH, SP_3).0,
            count,
            "{width}x{height}"
        );
    }
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    vcx.simulate_resize(size(px(1280.), px(800.)));
    let tile = super::bounds(vcx, "tile-output").unwrap();
    let (_, column) = columns(px(1280.) - SP_4 * 2., TILE_WIDTH, SP_3);
    assert!(
        (tile.size.width - column).abs() < px(1.),
        "a fifth of the page"
    );
    assert!(tile.right() <= px(1280.), "nothing sideways");
}

#[gpui::test]
fn a_tap_presses_like_a_click(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (_deck, vcx) = open(cx, &mock);
    tap(vcx, "tile-output");
    assert_eq!(mock.state().presses, ["output"]);
    assert!(shows(vcx, "BTイヤホン (JBL Tour Pro 3)"), "pressed");
    // GPUI keeps no hover after a touch (it draws hover for the mouse only).
    vcx.update(|window, _| assert!(window.last_input_was_touch(), "the last input was a finger"));
}

#[gpui::test]
fn the_cursor_highlights_menu_chooses_its_colour(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    mock.state().buttons = vec![highlight_button("blue")];
    let (_deck, vcx) = open(cx, &mock);
    assert!(shows(vcx, "目立たせる"), "its state");
    right_click(vcx, "tile-cursor");
    assert!(
        has(vcx, "menu-赤") && has(vcx, "menu-青（選択中）") && has(vcx, "menu-緑"),
        "the colours, blue chosen"
    );
    click(vcx, "menu-赤");
    assert!(!has(vcx, "menu-赤"), "closed");
    assert_eq!(mock.state().colors, ["red"]);
    // A right click chooses; it does not press.
    assert!(mock.state().presses.is_empty(), "not pressed");
    long_press(vcx, "tile-cursor");
    assert!(
        has(vcx, "menu-赤（選択中）") && has(vcx, "menu-青"),
        "red now chosen, from a long press"
    );
    assert!(
        mock.state().presses.is_empty(),
        "the long press did not press"
    );
}

#[gpui::test]
fn the_output_shows_its_device_icon_and_follows(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    let (deck, vcx) = open(cx, &mock);
    let link = super::link(&deck, vcx);
    let icon = |vcx: &mut gpui::VisualTestContext| {
        link.read_with(vcx, |l, _| {
            l.buttons
                .iter()
                .find(|b| b.id == "output")
                .and_then(|b| crate::model::display::state_icon(&b.state))
        })
    };
    assert_eq!(icon(vcx), Some(crate::model::display::Glyph::Speaker));
    mock.change(output("jbl"));
    vcx.run_until_parked();
    assert_eq!(icon(vcx), Some(crate::model::display::Glyph::Headphones));
}
