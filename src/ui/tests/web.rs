//! A library whose items open on the web (`DLsite`'s works that are not games): nothing is on
//! this PC, so the panel says a work opened, offers no folder, and speaks of works.

use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::{click, has, keys, long_press, open, right_click, shows};
use crate::link::mock::{MockLink, web_button, web_works};

fn mock(pins: Value) -> MockLink {
    let mock = MockLink::default();
    {
        let mut state = mock.state();
        state.buttons.push(web_button(pins));
        state.dlsite = web_works();
    }
    mock
}

fn work(id: &str) -> String {
    format!("tile-list-dlsite/{id}")
}

fn open_list<'a>(cx: &'a mut TestAppContext, mock: &MockLink) -> &'a mut VisualTestContext {
    let (_deck, vcx) = open(cx, mock);
    click(vcx, "tile-dlsite");
    vcx
}

fn offers_folder(vcx: &mut VisualTestContext) -> bool {
    has(vcx, "menu-ローカルファイル閲覧") || has(vcx, "disabled-ローカルファイル閲覧")
}

#[gpui::test]
fn a_works_menu_offers_no_folder_also_on_top(cx: &mut TestAppContext) {
    let mock = mock(json!([{ "id": "RJ1", "name": "耳かきの音" }]));
    let vcx = open_list(cx, &mock);
    right_click(vcx, &work("RJ1"));
    assert!(
        has(vcx, "menu-TOPから外す")
            && has(vcx, "menu-販売ページを開く")
            && has(vcx, "menu-ラベル設定"),
        "the rest of the menu"
    );
    assert!(!offers_folder(vcx), "nothing is on this PC");
    keys(vcx, "escape escape");
    right_click(vcx, "tile-dlsite/RJ1");
    assert!(has(vcx, "menu-TOPから外す"), "the pinned work's menu");
    assert!(!offers_folder(vcx), "nor on TOP");
}

#[gpui::test]
fn a_tapped_work_is_said_to_have_opened(cx: &mut TestAppContext) {
    let mock = mock(json!([]));
    let vcx = open_list(cx, &mock);
    click(vcx, &work("RJ1"));
    assert!(
        shows(vcx, "「耳かきの音」を開きました"),
        "opened, not started"
    );
    assert_eq!(mock.state().started, ["RJ1"]);
}

#[gpui::test]
fn before_signing_in_the_list_says_the_works_will_show(cx: &mut TestAppContext) {
    let mock = mock(json!([]));
    {
        let mut state = mock.state();
        let last = state.buttons.len() - 1;
        state.buttons[last]["state"]["sign_in"] = json!("dlsite");
        state.dlsite["sign_in"] = json!("dlsite");
    }
    let vcx = open_list(cx, &mock);
    assert!(
        shows(
            vcx,
            "DLsite にログインしていません。ログインすると、買った作品の一覧が出ます（ログイン画面はメインモニターに開きます）。"
        ),
        "nothing is downloaded"
    );
}

#[gpui::test]
fn deleting_a_label_says_the_works_stay(cx: &mut TestAppContext) {
    let mock = mock(json!([]));
    let vcx = open_list(cx, &mock);
    long_press(vcx, "chip-ASMR");
    click(vcx, "menu-削除");
    assert!(
        shows(vcx, "ラベルだけを消します。作品はライブラリに残ります。"),
        "works, not games"
    );
}

#[gpui::test]
fn the_lists_own_order_is_the_works_opened_last(cx: &mut TestAppContext) {
    let mock = mock(json!([]));
    let vcx = open_list(cx, &mock);
    click(vcx, "order");
    assert!(has(vcx, "checked-最近開いた順"), "the current order");
    assert!(
        has(vcx, "menu-更新日の新しい順") && has(vcx, "menu-購入日の新しい順"),
        "the orders by date stay"
    );
    assert!(!has(vcx, "menu-最近遊んだ順"), "a work is not played here");
}
