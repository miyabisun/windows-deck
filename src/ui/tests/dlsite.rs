//! A `DLsite` library: makers, downloads under way, programs to choose, license keys, the orders
//! by date, shop pages and the round of updates (`e2e/dlsite.spec.js`).

use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::{click, has, keys, long_press, open, right_click, shows};
use crate::{
    link::mock::{MockLink, dlsite_button},
    model::library::date_label,
    ui::theme::ROW_HEIGHT,
};

fn mock() -> MockLink {
    let mock = MockLink::default();
    mock.state().buttons.push(dlsite_button(json!([])));
    mock
}

fn game(id: &str) -> String {
    format!("tile-list-dlsite/{id}")
}

/// The games' order on the screen, top to bottom and left to right.
fn order(vcx: &mut VisualTestContext, ids: &[&str]) -> Vec<String> {
    let mut placed: Vec<(f32, f32, String)> = ids
        .iter()
        .filter_map(|id| {
            super::bounds(vcx, &game(id))
                .map(|b| (f32::from(b.top()), f32::from(b.left()), (*id).to_owned()))
        })
        .collect();
    placed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    placed.into_iter().map(|p| p.2).collect()
}

fn open_list<'a>(cx: &'a mut TestAppContext, mock: &MockLink) -> &'a mut VisualTestContext {
    let (_deck, vcx) = open(cx, mock);
    click(vcx, "tile-dlsite");
    vcx
}

fn day(seconds: i64) -> String {
    date_label(seconds, &chrono::Local)
}

#[gpui::test]
fn games_show_their_maker_and_the_maker_can_be_searched(cx: &mut TestAppContext) {
    let mock = mock();
    let vcx = open_list(cx, &mock);
    assert_eq!(
        order(vcx, &["a1", "b2", "c3"]),
        ["a1", "b2", "c3"],
        "windows-link's order"
    );
    assert!(shows(vcx, "3Djp_Art"), "the maker under the title");
    vcx.simulate_input("saimin");
    assert_eq!(
        order(vcx, &["a1", "b2", "c3"]),
        ["b2"],
        "found by its maker"
    );
}

#[gpui::test]
fn a_game_not_downloaded_yet_says_how_its_download_goes(cx: &mut TestAppContext) {
    let mock = mock();
    let coming = json!({ "id": "RJ7", "name": "まだ来ないゲーム", "detail": "Brand", "choosable": false, "image": null,
        "installed": false, "labels": [], "status": "ダウンロード中 40%" });
    if let Some(items) = mock.state().dlsite["items"].as_array_mut() {
        items.insert(0, coming);
    }
    let vcx = open_list(cx, &mock);
    assert!(
        shows(vcx, "Brand・ダウンロード中 40%"),
        "how it goes, not 未インストール"
    );
    click(vcx, &game("RJ7"));
    assert!(
        shows(vcx, "まだこの PC にありません（ダウンロード中 40%）"),
        "why it does not start"
    );
    assert!(mock.state().started.is_empty(), "not started");
}

#[gpui::test]
fn enter_in_the_search_field_starts_the_first_game_found(cx: &mut TestAppContext) {
    let mock = mock();
    let vcx = open_list(cx, &mock);
    vcx.simulate_input("3djp");
    assert_eq!(order(vcx, &["a1", "b2", "c3"]), ["a1"]);
    keys(vcx, "enter");
    assert!(
        shows(vcx, "「湿度の高い夏のマゾ」を起動しました"),
        "said so"
    );
    assert_eq!(mock.state().started, ["a1"]);
}

#[gpui::test]
fn a_game_with_several_programs_asks_once_which_one_starts_it(cx: &mut TestAppContext) {
    let mock = mock();
    let vcx = open_list(cx, &mock);
    click(vcx, &game("b2"));
    assert!(shows(vcx, "起動ファイルを選ぶ: 催眠アプリ"), "asks");
    let row = super::bounds(vcx, "row-app.exe").expect("a program");
    assert!(row.size.height >= ROW_HEIGHT, "a finger's row");
    click(vcx, "row-startup.exe");
    assert!(!has(vcx, "row-startup.exe"), "closed");
    assert!(shows(vcx, "「催眠アプリ」を起動しました"), "started");
    assert_eq!(mock.state().started, ["b2:startup.exe"]);
    // Next time it starts at once.
    click(vcx, &game("b2"));
    assert_eq!(mock.state().started, ["b2:startup.exe", "b2:startup.exe"]);
    assert!(!has(vcx, "row-startup.exe"), "no question");
}

#[gpui::test]
fn a_games_menu_changes_its_program_only_when_it_has_a_choice(cx: &mut TestAppContext) {
    let mock = mock();
    let vcx = open_list(cx, &mock);
    long_press(vcx, &game("a1"));
    assert!(!has(vcx, "menu-起動ファイルを選ぶ"), "a1 has one program");
    keys(vcx, "escape");
    mock.state().dlsite["programs"]["b2"]["chosen"] = json!("app.exe");
    long_press(vcx, &game("b2"));
    click(vcx, "menu-起動ファイルを選ぶ");
    assert!(has(vcx, "on-app.exe"), "the one in use");
    click(vcx, "row-startup.exe");
    assert_eq!(
        mock.state().dlsite["programs"]["b2"]["chosen"],
        "startup.exe"
    );
    // Choosing from the menu does not start the game.
    assert!(mock.state().started.is_empty(), "not started");
}

#[gpui::test]
fn a_games_license_key_is_shown_with_a_button_that_copies_it(cx: &mut TestAppContext) {
    let mock = mock();
    let vcx = open_list(cx, &mock);
    long_press(vcx, &game("a1"));
    click(vcx, "menu-シリアル番号");
    assert!(
        shows(vcx, "シリアル番号: 湿度の高い夏のマゾ") && shows(vcx, "ABCD-1234-EFGH-5678"),
        "the key, large"
    );
    let copy = super::bounds(vcx, "button-コピー").expect("copy");
    assert!(copy.size.height >= ROW_HEIGHT, "a finger's button");
    click(vcx, "button-コピー");
    let copied = vcx.update(|_, cx| cx.read_from_clipboard().and_then(|c| c.text()));
    assert_eq!(copied.as_deref(), Some("ABCD-1234-EFGH-5678"));
    assert!(shows(vcx, "コピーしました"), "said so");
    click(vcx, "close");
    assert!(!shows(vcx, "ABCD-1234-EFGH-5678"), "closed");
    long_press(vcx, &game("b2"));
    click(vcx, "menu-シリアル番号");
    assert!(shows(vcx, "このゲームにはシリアル番号がありません"), "none");
}

#[gpui::test]
fn the_list_goes_in_order_of_update_or_purchase_and_stays_so(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-dlsite");
    assert_eq!(order(vcx, &["a1", "b2", "c3"]), ["a1", "b2", "c3"]);
    click(vcx, "order");
    assert!(has(vcx, "checked-最近遊んだ順"), "the current order");
    for row in [
        "menu-最近遊んだ順",
        "menu-更新日の新しい順",
        "menu-購入日の新しい順",
    ] {
        assert!(
            super::bounds(vcx, row).unwrap().size.height >= ROW_HEIGHT,
            "{row} is a finger's row"
        );
    }
    click(vcx, "menu-更新日の新しい順");
    assert!(!has(vcx, "menu-最近遊んだ順"), "the menu closed");
    assert_eq!(order(vcx, &["a1", "b2", "c3"]), ["b2", "a1", "c3"]);
    // The date the list is in order of heads each note.
    assert!(
        shows(vcx, &format!("更新 {}・Saimin Soft", day(1_793_118_930))),
        "b2's date"
    );
    assert!(
        shows(vcx, &format!("更新 {}・3Djp_Art", day(1_792_076_400))),
        "a1's date"
    );
    assert!(shows(vcx, "Other"), "c3 has none");
    click(vcx, "order");
    click(vcx, "menu-購入日の新しい順");
    assert_eq!(order(vcx, &["a1", "b2", "c3"]), ["a1", "b2", "c3"]);
    assert!(
        shows(vcx, &format!("購入 {}・3Djp_Art", day(1_767_290_645))),
        "a1's purchase"
    );
    assert!(shows(vcx, "Saimin Soft"), "b2 has no purchase date");
    // Opened again, the list keeps its order.
    keys(vcx, "escape");
    click(vcx, "tile-dlsite");
    assert!(
        shows(vcx, &format!("購入 {}・3Djp_Art", day(1_767_290_645))),
        "kept"
    );
}

#[gpui::test]
fn a_games_menu_opens_its_dlsite_page_also_from_top(cx: &mut TestAppContext) {
    let mock = MockLink::default();
    mock.state().buttons.push(dlsite_button(
        json!([{ "id": "a1", "name": "湿度の高い夏のマゾ" }]),
    ));
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-dlsite");
    right_click(vcx, &game("a1"));
    click(vcx, "menu-販売ページを開く");
    assert_eq!(mock.state().pages, ["a1"]);
    // A game not known on DLsite has no page to open.
    right_click(vcx, &game("c3"));
    assert!(
        has(vcx, "menu-TOPに固定") && !has(vcx, "menu-販売ページを開く"),
        "no page for c3"
    );
    keys(vcx, "escape escape");
    right_click(vcx, "tile-dlsite/a1");
    click(vcx, "menu-販売ページを開く");
    assert_eq!(mock.state().pages, ["a1", "a1"]);
}

#[gpui::test]
fn the_library_buttons_menu_shows_how_the_update_goes(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    right_click(vcx, "tile-dlsite");
    assert!(
        has(vcx, "menu-ライブラリを更新") && has(vcx, "menu-更新の状況"),
        "both"
    );
    click(vcx, "menu-更新の状況");
    assert!(shows(vcx, "更新の状況: DLsite"), "its dialog");
    assert!(
        shows(
            vcx,
            "購入一覧を読めませんでした（DLsite のログイン画面が HTTP 403 を返しました）。新しいゲームと更新は探せていません"
        ),
        "why"
    );
    // It follows the round while open.
    mock.state().dlsite_round = json!({ "doing": "downloads", "started": 1_793_119_000, "finished": null, "purchases": null, "jobs": [
        { "id": "RJ7", "name": "来たゲーム", "status": "完了" },
        { "id": "RJ8", "name": "来るゲーム", "status": "ダウンロード中 40%" } ] });
    vcx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    assert!(
        shows(vcx, "ダウンロードと更新: 2 件のうち 1 件が完了"),
        "followed"
    );
    assert!(
        shows(vcx, "来たゲーム") && shows(vcx, "ダウンロード中 40%"),
        "each game"
    );
    click(vcx, "button-今すぐ更新");
    assert!(mock.state().updates.is_empty(), "not while the round runs");
    // Once it ends, it can be started again from here.
    {
        let mut state = mock.state();
        state.dlsite_round["doing"] = Value::Null;
        state.dlsite_round["finished"] = json!(1_793_119_600);
        state.update_answer = Some((202, json!({ "round": true })));
    }
    vcx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    click(vcx, "button-今すぐ更新");
    assert_eq!(mock.state().updates, ["dlsite"]);
    click(vcx, "close");
    assert!(!shows(vcx, "更新の状況: DLsite"), "closed");
}

#[gpui::test]
fn before_signing_in_to_dlsite_its_button_and_list_say_so(cx: &mut TestAppContext) {
    let mock = mock();
    {
        let mut state = mock.state();
        let last = state.buttons.len() - 1;
        state.buttons[last]["state"]["sign_in"] = json!("dlsite");
        state.dlsite["sign_in"] = json!("dlsite");
    }
    let (_deck, vcx) = open(cx, &mock);
    assert!(shows(vcx, "ログインが必要です"), "on its button");
    click(vcx, "tile-dlsite");
    assert!(
        shows(
            vcx,
            "DLsite にログインしていません。ログインすると、買ったゲームの一覧とダウンロードが始まります（ログイン画面はメインモニターに開きます）。"
        ),
        "in its list"
    );
}

#[gpui::test]
fn a_game_without_a_program_says_so_on_its_tile(cx: &mut TestAppContext) {
    let mock = mock();
    let vcx = open_list(cx, &mock);
    click(vcx, &game("c3"));
    assert!(
        shows(
            vcx,
            "起動できるファイルが見つかりません（ローカルファイル閲覧で中を確かめてください）"
        ),
        "why"
    );
}
