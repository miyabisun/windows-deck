//! A Steam library's list: search, labels, a game's menu, dragging onto a label, signing in
//! and updating (`e2e/library.spec.js`).

use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, TestAppContext, VisualTestContext, px};
use serde_json::{Value, json};

use super::{click, has, keys, long_press, open, right_click, shows};
use crate::{
    link::mock::{MockLink, dlsite_button, library_button},
    ui::theme::ROW_HEIGHT,
};

fn mock() -> MockLink {
    let mock = MockLink::default();
    mock.state().buttons.push(library_button(json!([])));
    mock
}

fn game(id: &str) -> String {
    format!("tile-list-games/{id}")
}

fn listed(vcx: &mut VisualTestContext, ids: &[&str], absent: &[&str]) -> bool {
    ids.iter().all(|id| has(vcx, &game(id))) && absent.iter().all(|id| !has(vcx, &game(id)))
}

/// The list starts with only the unlabeled games; show every game instead.
fn show_all(vcx: &mut VisualTestContext) {
    click(vcx, "chip-ラベル非登録");
}

fn type_text(vcx: &mut VisualTestContext, text: &str) {
    vcx.simulate_input(text);
    vcx.run_until_parked();
}

fn clear_search(vcx: &mut VisualTestContext) {
    for _ in 0..20 {
        keys(vcx, "backspace");
    }
}

fn labels_of(mock: &MockLink, index: usize) -> Value {
    mock.state().library["items"][index]["labels"].clone()
}

/// Drag with the mouse from one element onto another.
fn drag(vcx: &mut VisualTestContext, from: &str, to: &str) {
    let start = super::bounds(vcx, from).unwrap().center();
    let end = super::bounds(vcx, to).unwrap().center();
    let modifiers = Modifiers::none();
    vcx.simulate_event(MouseDownEvent { button: MouseButton::Left, position: start, modifiers, click_count: 1, first_mouse: false });
    for step in 1..=10 {
        #[allow(clippy::cast_precision_loss)]
        let part = step as f32 / 10.;
        let at = gpui::point(start.x + (end.x - start.x) * part, start.y + (end.y - start.y) * part);
        vcx.simulate_event(MouseMoveEvent { position: at, pressed_button: Some(MouseButton::Left), modifiers });
    }
    vcx.simulate_event(MouseUpEvent { button: MouseButton::Left, position: end, modifiers, click_count: 1 });
    vcx.run_until_parked();
}

#[gpui::test]
fn opens_at_80_percent_with_the_search_field_focused(cx: &mut TestAppContext) {
    let mock = mock();
    let (deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    let panel = super::bounds(vcx, "search").expect("the search field");
    assert!(panel.size.height >= px(56.), "a finger's height");
    let library = deck.read_with(vcx, |d, _| d.library()).expect("open");
    let search = library.read_with(vcx, |l, cx| l.search_focus(cx));
    vcx.update(|window, _| assert!(search.is_focused(window), "the search field has the focus"));
    // It starts with the games that have no label yet.
    assert!(has(vcx, "chip-ラベル非登録"), "ラベル非登録 first");
    assert!(listed(vcx, &["646570"], &["1364780", "2868840", "1039890"]), "only the unlabeled");
    show_all(vcx);
    // 非表示 is hidden until its label is selected.
    assert!(listed(vcx, &["1364780", "646570", "2868840"], &["1039890"]), "all but the hidden");
    assert!(shows(vcx, "未インストール（押すとインストール）"), "not installed");
    for chip in ["chip-ラベル非登録", "chip-お気に入り", "chip-非表示", "chip-R15", "add-label"] {
        assert!(super::bounds(vcx, chip).unwrap().size.height >= px(48.), "{chip} is big enough");
    }
    keys(vcx, "escape");
    assert!(deck.read_with(vcx, |d, _| d.library()).is_none(), "Esc closes it");
}

#[gpui::test]
fn unlabeled_and_the_other_labels_are_not_selected_together(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    click(vcx, "chip-R15");
    assert!(listed(vcx, &["2868840"], &["646570"]), "R15 alone");
    click(vcx, "chip-ラベル非登録");
    assert!(listed(vcx, &["646570"], &["2868840"]), "ラベル非登録 alone again");
}

#[gpui::test]
fn typing_and_labels_narrow_the_list(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    show_all(vcx);
    click(vcx, "search");
    type_text(vcx, "spire");
    assert!(listed(vcx, &["646570", "2868840"], &["1364780"]), "by name");
    click(vcx, "chip-R15");
    assert!(listed(vcx, &["2868840"], &["646570"]), "and label");
    click(vcx, "search");
    clear_search(vcx);
    click(vcx, "chip-R15");
    click(vcx, "chip-非表示");
    assert!(listed(vcx, &["1039890"], &["646570"]), "the hidden label's games");
    click(vcx, "search");
    type_text(vcx, "zelda");
    assert!(shows(vcx, "見つかりません"), "nothing found");
}

#[gpui::test]
fn tapping_a_game_starts_it_and_leaves_the_list_open(cx: &mut TestAppContext) {
    let mock = mock();
    mock.state().library["items"] = Value::Array((0..60).map(|i| json!({ "id": (1000 + i).to_string(), "name": format!("Game {i}"), "installed": true, "labels": [] })).collect());
    let (deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    super::tap(vcx, &game("1003"));
    assert!(shows(vcx, "「Game 3」を起動しました"), "said so");
    assert_eq!(mock.state().started, ["1003"]);
    assert!(deck.read_with(vcx, |d, _| d.library()).is_some(), "still open");
    // A tap outside the list closes it.
    vcx.simulate_click(gpui::point(px(5.), px(5.)), Modifiers::none());
    vcx.run_until_parked();
    assert!(deck.read_with(vcx, |d, _| d.library()).is_none(), "closed");
}

#[gpui::test]
fn a_long_list_scrolls_with_a_finger_and_keeps_its_place(cx: &mut TestAppContext) {
    let mock = mock();
    mock.state().library["items"] = Value::Array((0..60).map(|i| json!({ "id": (1000 + i).to_string(), "name": format!("Game {i}"), "installed": true, "labels": [] })).collect());
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    assert!(has(vcx, &game("1000")) && !has(vcx, &game("1059")), "the first rows only");
    for _ in 0..8 {
        super::slide(vcx, "games-area", 0., -600.);
        if has(vcx, &game("1059")) {
            break;
        }
    }
    assert!(has(vcx, &game("1059")), "the last game came into view");
    let before = super::bounds(vcx, &game("1059")).unwrap();
    // As on a phone, the first touch after a fling only stops the list.
    super::tap(vcx, &game("1059"));
    assert!(mock.state().started.is_empty(), "the slide and the stop started nothing");
    super::tap(vcx, &game("1059"));
    assert_eq!(mock.state().started, ["1059"], "a tap starts it");
    assert_eq!(super::bounds(vcx, &game("1059")).unwrap(), before, "the list stays where it was");
}

#[gpui::test]
fn a_games_menu_pins_it_and_the_pinned_tiles_menu_takes_it_off(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    long_press(vcx, &game("646570"));
    let row = super::bounds(vcx, "menu-TOPに固定").expect("its menu");
    assert!(row.size.height >= ROW_HEIGHT, "a finger's row");
    click(vcx, "menu-TOPに固定");
    assert!(shows(vcx, "「Slay the Spire」を TOP に固定しました"), "said so");
    assert!(shows(vcx, "TOP に固定中"), "noted on the tile");
    assert!(mock.state().started.is_empty(), "the long press did not start it");
    click(vcx, "close");
    assert!(has(vcx, "tile-games/646570"), "pinned to the tab");
    click(vcx, "tile-games/646570");
    assert_eq!(mock.state().started, ["646570"]);
    long_press(vcx, "tile-games/646570");
    click(vcx, "menu-TOPから外す");
    assert!(!has(vcx, "tile-games/646570"), "taken off");
    assert_eq!(mock.state().started, ["646570"]);
}

#[gpui::test]
fn a_games_menu_opens_its_folder_only_when_installed(cx: &mut TestAppContext) {
    let mock = mock();
    let (deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    long_press(vcx, &game("646570"));
    click(vcx, "menu-ローカルファイル閲覧");
    assert_eq!(mock.state().folders, ["646570"]);
    show_all(vcx);
    long_press(vcx, &game("2868840"));
    assert!(has(vcx, "disabled-ローカルファイル閲覧"), "not installed: cannot open");
    keys(vcx, "escape");
    assert!(!has(vcx, "menu-TOPに固定"), "the menu closed");
    assert!(deck.read_with(vcx, |d, _| d.library()).is_some(), "the list stays");
}

#[gpui::test]
fn a_games_labels_are_set_from_its_menu_including_a_new_one(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    long_press(vcx, &game("646570"));
    click(vcx, "menu-ラベル設定");
    assert!(shows(vcx, "ラベル設定: Slay the Spire"), "its labels");
    assert!(!has(vcx, "on-非表示"), "not hidden yet");
    for row in ["row-お気に入り", "row-非表示", "row-R15", "row-新しいラベル"] {
        assert!(super::bounds(vcx, row).unwrap().size.height >= ROW_HEIGHT, "{row} is a finger's row");
    }
    click(vcx, "row-非表示");
    assert!(has(vcx, "on-非表示"), "checked");
    assert_eq!(labels_of(&mock, 1), json!(["hidden"]));
    click(vcx, "row-新しいラベル");
    type_text(vcx, "ローグライク");
    click(vcx, "button-作って付ける");
    assert!(has(vcx, "on-ローグライク"), "made and put on");
    click(vcx, "close");
    // The list follows: the game is now hidden, and the new label has a chip.
    assert!(!has(vcx, &game("646570")), "hidden now");
    assert!(has(vcx, "chip-ローグライク"), "a chip for the new label");
}

#[gpui::test]
fn the_plus_chip_makes_a_label_and_its_menu_renames_or_deletes_it(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    click(vcx, "add-label");
    type_text(vcx, "RPG");
    click(vcx, "button-作る");
    assert!(has(vcx, "chip-RPG"), "made");
    long_press(vcx, "chip-RPG");
    click(vcx, "menu-名前変更");
    // The field starts with the name; replace it.
    for _ in 0..3 {
        keys(vcx, "backspace");
    }
    type_text(vcx, "JRPG");
    click(vcx, "button-変更する");
    assert!(has(vcx, "chip-JRPG") && !has(vcx, "chip-RPG"), "renamed");
    long_press(vcx, "chip-JRPG");
    click(vcx, "menu-削除");
    assert!(shows(vcx, "ラベルだけを消します。ゲームはライブラリに残ります。"), "asks first");
    click(vcx, "button-削除する");
    assert!(!has(vcx, "chip-JRPG"), "deleted");
    // Steam's own labels cannot be renamed or deleted, and a long press does not toggle.
    long_press(vcx, "chip-非表示");
    assert!(has(vcx, "disabled-名前変更") && has(vcx, "disabled-削除"), "Steam's own");
    keys(vcx, "escape");
    assert!(has(vcx, &game("646570")) && !has(vcx, &game("1039890")), "非表示 still off");
}

#[gpui::test]
fn while_labels_cannot_be_changed_the_panel_says_why(cx: &mut TestAppContext) {
    let mock = mock();
    mock.state().library["labels_locked"] = json!("Steam is not running");
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    click(vcx, "add-label");
    assert!(!has(vcx, "button-作る"), "+ does nothing");
    long_press(vcx, &game("646570"));
    click(vcx, "menu-ラベル設定");
    assert!(shows(vcx, "Steam が起動していないため、ラベルは変更できません"), "why");
    click(vcx, "row-非表示");
    assert_eq!(labels_of(&mock, 1), json!([]), "unchanged");
}

#[gpui::test]
fn a_right_click_opens_the_same_menus_as_a_long_press(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    right_click(vcx, &game("646570"));
    click(vcx, "menu-TOPに固定");
    assert!(mock.state().started.is_empty(), "not started");
    right_click(vcx, "chip-R15");
    assert!(has(vcx, "menu-名前変更"), "the label's menu");
    keys(vcx, "escape");
    // A right click neither toggles a label nor starts a game.
    assert!(listed(vcx, &["646570"], &["2868840"]), "R15 not toggled");
    click(vcx, "close");
    right_click(vcx, "tile-games/646570");
    assert!(has(vcx, "menu-TOPから外す"), "the pinned tile's menu");
    assert!(mock.state().started.is_empty(), "still not started");
}

#[gpui::test]
fn steam_games_have_no_license_keys(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    long_press(vcx, &game("646570"));
    assert!(has(vcx, "menu-TOPに固定") && !has(vcx, "menu-シリアル番号"), "no keys for Steam");
}

#[gpui::test]
fn dragging_a_game_onto_a_label_puts_it_there(cx: &mut TestAppContext) {
    let mock = mock();
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    drag(vcx, &game("646570"), "chip-R15");
    assert!(shows(vcx, "「Slay the Spire」に「R15」を付けました"), "said so");
    assert_eq!(labels_of(&mock, 1), json!(["uc-r15"]));
    // It had no label, so the list that starts with the unlabeled games lets it go.
    assert!(shows(vcx, "見つかりません"), "gone from ラベル非登録");
    assert!(mock.state().started.is_empty(), "not started");
    show_all(vcx);
    drag(vcx, &game("2868840"), "chip-R15");
    assert!(shows(vcx, "「Slay the Spire 2」にはもう「R15」が付いています"), "already there");
    drag(vcx, &game("1364780"), "chip-非表示");
    assert!(!has(vcx, &game("1364780")), "hidden now");
}

#[gpui::test]
fn while_labels_cannot_be_changed_a_game_cannot_be_dropped(cx: &mut TestAppContext) {
    let mock = mock();
    mock.state().library["labels_locked"] = json!("Steam is not running");
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    drag(vcx, &game("646570"), "chip-R15");
    assert_eq!(labels_of(&mock, 1), json!([]));
    assert!(has(vcx, &game("646570")), "still listed");
}

#[gpui::test]
fn a_library_of_installed_games_only_says_why(cx: &mut TestAppContext) {
    let mock = mock();
    mock.state().library["partial"] = json!("secrets.yaml has no steam api_key");
    let (_deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    assert!(shows(vcx, "windows-link の secrets.yaml に Steam の API キーが無いため、インストール済みのゲームだけを出しています"), "why");
}

#[gpui::test]
fn a_shop_that_needs_signing_in_says_so_and_signs_in(cx: &mut TestAppContext) {
    let mock = mock();
    {
        let mut state = mock.state();
        let last = state.buttons.len() - 1;
        state.buttons[last]["state"]["sign_in"] = json!("fanza");
        state.library["sign_in"] = json!("fanza");
    }
    let (_deck, vcx) = open(cx, &mock);
    assert!(shows(vcx, "ログインが必要です"), "on its button");
    click(vcx, "tile-games");
    assert!(shows(vcx, "FANZA にログインしていません。ログインすると、買ったゲームの一覧とダウンロードが始まります（ログイン画面はメインモニターに開きます）。"), "in its list");
    assert!(super::bounds(vcx, "sign-in").unwrap().size.height >= px(48.), "a finger's button");
    // Closed before signing in.
    click(vcx, "sign-in");
    assert!(shows(vcx, "ログインせずにログイン画面が閉じられました"), "why");
    // Signed in: the cookies go to windows-link and the list is read again.
    mock.state().login = Ok(json!([{ "name": "login_secure_id", "value": "v", "domain": "dmm.co.jp" }]));
    mock.state().library["sign_in"] = Value::Null;
    click(vcx, "sign-in");
    assert_eq!(mock.state().sessions.len(), 1, "handed over");
    assert_eq!(mock.state().sessions[0].0, "fanza");
    assert!(!has(vcx, "sign-in"), "signed in");
}

#[gpui::test]
fn a_library_buttons_menu_updates_it_and_says_what_started(cx: &mut TestAppContext) {
    let mock = mock();
    let (deck, vcx) = open(cx, &mock);
    right_click(vcx, "tile-games");
    assert!(deck.read_with(vcx, |d, _| d.library()).is_none(), "a right click does not open the list");
    assert!(super::bounds(vcx, "menu-ライブラリを更新").unwrap().size.height >= ROW_HEIGHT, "a finger's row");
    // Steam's updates have no rounds to follow.
    assert!(!has(vcx, "menu-更新の状況"), "no rounds");
    click(vcx, "menu-ライブラリを更新");
    assert!(shows(vcx, "アップデート 3 件を始めました。インストール 24 件は Steam の画面で確定してください"), "what started");
    assert_eq!(mock.state().updates, ["games"]);
    // It goes after a few seconds.
    vcx.executor().advance_clock(std::time::Duration::from_secs(4));
    assert!(!shows(vcx, "アップデート 3 件を始めました。インストール 24 件は Steam の画面で確定してください"), "gone");
    long_press(vcx, "tile-games");
    assert!(has(vcx, "menu-ライブラリを更新") && deck.read_with(vcx, |d, _| d.library()).is_none(), "a long press opens the menu, not the list");
    keys(vcx, "escape");
    assert!(!has(vcx, "menu-ライブラリを更新"), "closed");
}

#[gpui::test]
fn a_library_that_cannot_be_updated_says_why_until_opened(cx: &mut TestAppContext) {
    let mock = mock();
    mock.state().update_answer = Some((409, json!({ "error": "update_unavailable", "message": "Steam is not running" })));
    let (deck, vcx) = open(cx, &mock);
    right_click(vcx, "tile-games");
    click(vcx, "menu-ライブラリを更新");
    assert!(shows(vcx, "Steam が起動していないため、ライブラリを更新できません"), "why, on the button");
    assert!(has(vcx, "failed-games"), "failed");
    click(vcx, "tile-games");
    assert!(deck.read_with(vcx, |d, _| d.library()).is_some(), "opened");
    keys(vcx, "escape");
    assert!(!shows(vcx, "Steam が起動していないため、ライブラリを更新できません"), "cleared by opening it");
}

#[gpui::test]
fn a_shops_update_starts_its_downloads_and_shows_how_they_go(cx: &mut TestAppContext) {
    let mock = mock();
    {
        let mut state = mock.state();
        state.buttons.push(dlsite_button(json!([])));
        state.update_answer = Some((202, json!({ "round": true })));
    }
    let (_deck, vcx) = open(cx, &mock);
    right_click(vcx, "tile-dlsite");
    click(vcx, "menu-ライブラリを更新");
    assert_eq!(mock.state().updates, ["dlsite"]);
    assert!(shows(vcx, "更新の状況: DLsite"), "its round opens");
}

