//! Searching and ordering a windows-link game library (`GET /buttons/{id}/library`).

use chrono::TimeZone;
use unicode_normalization::UnicodeNormalization;

use super::display::local;
use super::protocol::{Button, Item, Pin, State};

/// The label that stands for "no label at all"; a list starts with it selected, so the games
/// still to sort come first. Real label IDs never start with "@".
pub const UNLABELED: &str = "@unlabeled";

/// The shop `sign_in` names, as the panel writes it.
pub fn shop_name(shop: &str) -> &str {
    match shop {
        "fanza" => "FANZA",
        "dlsite" => "DLsite",
        other => other,
    }
}

/// An order a list can be in: windows-link's own (the games played last first, then those
/// installed last), or newest first by a date windows-link gives a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Order {
    Recent,
    Updated,
    Bought,
}

impl Order {
    pub const ALL: [Self; 3] = [Self::Recent, Self::Updated, Self::Bought];

    pub fn label(self) -> &'static str {
        match self {
            Self::Recent => "最近遊んだ順",
            Self::Updated => "更新日の新しい順",
            Self::Bought => "購入日の新しい順",
        }
    }

    /// What heads a game's note while the list is in this order.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::Recent => None,
            Self::Updated => Some("更新"),
            Self::Bought => Some("購入"),
        }
    }

    /// The date of `item` this order goes by.
    pub fn date(self, item: &Item) -> Option<i64> {
        match self {
            Self::Recent => None,
            Self::Updated => item.updated,
            Self::Bought => item.bought,
        }
    }
}

/// The orders worth offering for `items`: windows-link's own, and those by a date some game
/// has (Steam's games have none).
pub fn orders_of(items: &[Item]) -> Vec<Order> {
    Order::ALL
        .into_iter()
        .filter(|o| *o == Order::Recent || items.iter().any(|i| o.date(i).is_some()))
        .collect()
}

/// `items` in `order`: as windows-link gave them, or newest first by that date, the games
/// without it after them in windows-link's order.
pub fn sort_items(items: &[Item], order: Order) -> Vec<&Item> {
    let mut dated: Vec<&Item> = items.iter().filter(|i| order.date(i).is_some()).collect();
    if order == Order::Recent {
        return items.iter().collect();
    }
    // A stable sort keeps windows-link's order among games of the same date.
    dated.sort_by_key(|i| std::cmp::Reverse(order.date(i)));
    dated.extend(items.iter().filter(|i| order.date(i).is_none()));
    dated
}

/// A date windows-link gives (Unix seconds) as the day it was, such as 2026/10/16.
pub fn date_label<Tz: TimeZone>(seconds: i64, zone: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    local(seconds, zone).map_or_else(String::new, |day| day.format("%Y/%m/%d").to_string())
}

/// Fold full-width letters and case so that "ｓｔｒｅｅｔ" finds "Street".
pub fn fold(text: &str) -> String {
    text.nfkc().collect::<String>().to_lowercase()
}

/// The labels chosen after tapping `label`'s chip: it is taken off when chosen, else added.
/// ラベル非登録 goes alone: choosing it takes the other labels off, and choosing another label
/// takes it off (a game without labels has none of them anyway).
pub fn toggle_label(active: &[String], label: &str) -> Vec<String> {
    if active.iter().any(|l| l == label) {
        return active.iter().filter(|l| *l != label).cloned().collect();
    }
    if label == UNLABELED {
        return vec![UNLABELED.into()];
    }
    let mut next: Vec<String> = active.iter().filter(|l| *l != UNLABELED).cloned().collect();
    next.push(label.into());
    next
}

/// What narrows a list.
#[derive(Debug, Clone, Copy, Default)]
pub struct Filter<'a> {
    pub query: &'a str,
    pub active: &'a [String],
    pub hide: &'a [String],
}

/// The games to show, like fzf: every word of the query is in the name or the detail (the
/// maker), or failing that its letters come there in order (those follow the others); with
/// labels selected, the game has one of them; and a game with a `hide` label shows only while
/// that label is selected.
pub fn visible_items<'a>(
    items: impl IntoIterator<Item = &'a Item>,
    filter: Filter<'_>,
) -> Vec<&'a Item> {
    let query = fold(filter.query);
    let words: Vec<&str> = query.split_whitespace().collect();
    let selected = |label: &String| filter.active.contains(label);
    let mut plain = Vec::new();
    let mut scattered = Vec::new();
    for item in items {
        let unlabeled = filter.active.iter().any(|l| l == UNLABELED) && item.labels.is_empty();
        if !filter.active.is_empty() && !unlabeled && !item.labels.iter().any(selected) {
            continue;
        }
        if item
            .labels
            .iter()
            .any(|label| filter.hide.contains(label) && !selected(label))
        {
            continue;
        }
        let text = match &item.detail {
            Some(detail) if !detail.is_empty() => fold(&format!("{} {detail}", item.name)),
            _ => fold(&item.name),
        };
        if words.iter().all(|word| text.contains(word)) {
            plain.push(item);
        } else if words.iter().all(|word| in_order(word, &text)) {
            scattered.push(item);
        }
    }
    plain.extend(scattered);
    plain
}

/// Whether the letters of `word` come in `text` in this order, with anything between.
fn in_order(word: &str, text: &str) -> bool {
    let mut rest = text;
    for letter in word.chars() {
        match rest.find(letter) {
            Some(at) => rest = &rest[at + letter.len_utf8()..],
            None => return false,
        }
    }
    true
}

/// Why the library lists only the installed games, in words the user can act on.
pub fn partial_reason(reason: Option<&str>) -> Option<String> {
    let reason = reason?;
    if reason.contains("no steam api_key") {
        return Some("windows-link の secrets.yaml に Steam の API キーが無いため、インストール済みのゲームだけを出しています".into());
    }
    if reason.contains("rejected the key") {
        return Some(
            "Steam が API キーを受け付けません。インストール済みのゲームだけを出しています".into(),
        );
    }
    if reason.contains("not fetched yet") {
        return Some("所有ゲームを取得中です。インストール済みのゲームだけを出しています".into());
    }
    if let Some(folder) = reason.strip_suffix(" does not exist") {
        return Some(format!("ゲームのフォルダ {folder} がありません"));
    }
    if let Some(folder) = reason.strip_prefix("there are no games in ") {
        return Some(format!("{folder} にゲームがありません"));
    }
    Some(format!(
        "インストール済みのゲームだけを出しています: {reason}"
    ))
}

/// Why labels cannot be changed right now, in words the user can act on.
pub fn labels_locked_reason(reason: Option<&str>) -> Option<String> {
    let reason = reason?;
    Some(
        steam_down(reason, "ラベルは変更できません")
            .unwrap_or_else(|| format!("ラベルを変更できません: {reason}")),
    )
}

/// Why windows-link cannot ask Steam, ending in `cannot` (what cannot be done), or none when
/// `reason` is about something else.
pub fn steam_down(reason: &str, cannot: &str) -> Option<String> {
    if reason.contains("not running") {
        return Some(format!("Steam が起動していないため、{cannot}"));
    }
    if reason.contains("cef-enable-remote-debugging") {
        return Some(format!(
            "Steam の操作口が無効なため、{cannot}（Steam フォルダに .cef-enable-remote-debugging を置いて Steam を再起動）"
        ));
    }
    None
}

/// A path segment as `encodeURIComponent` writes it.
pub fn encode(segment: &str) -> String {
    use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
    const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'!')
        .remove(b'~')
        .remove(b'*')
        .remove(b'\'')
        .remove(b'(')
        .remove(b')');
    utf8_percent_encode(segment, COMPONENT).to_string()
}

/// Where windows-link serves a library game's picture.
pub fn picture_url(base: &str, button: &str, item: &str) -> String {
    format!(
        "{base}/buttons/{}/library/{}/image",
        encode(button),
        encode(item)
    )
}

/// A game pinned to a library button, as a tab shows it after its buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    pub button: String,
    pub pin: Pin,
}

/// The games pinned to the library buttons among `buttons`, in button order.
pub fn pins_of<'a>(buttons: impl IntoIterator<Item = &'a Button>) -> Vec<Pinned> {
    buttons
        .into_iter()
        .flat_map(|b| match &b.state {
            State::Library(library) => library
                .pins
                .iter()
                .map(|pin| Pinned {
                    button: b.id.clone(),
                    pin: pin.clone(),
                })
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, Utc};

    use super::*;
    use crate::model::protocol::LibraryState;

    fn item(id: &str, name: &str, labels: &[&str]) -> Item {
        Item {
            id: id.into(),
            name: name.into(),
            labels: labels.iter().map(|l| (*l).into()).collect(),
            installed: Some(true),
            ..Item::default()
        }
    }

    fn items() -> Vec<Item> {
        vec![
            item("1", "Street Fighter™ 6", &[]),
            item("2", "Slay the Spire", &["お気に入り"]),
            item("3", "Slay the Spire 2", &["R15"]),
            item("4", "METAL SLUG", &["outdate"]),
            item("5", "Ｒｅｍｎａｎｔ", &["R15", "outdate"]),
        ]
    }

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).into()).collect()
    }

    fn ids(list: &[&Item]) -> Vec<String> {
        list.iter().map(|i| i.id.clone()).collect()
    }

    fn find(list: &[Item], query: &str, active: &[&str], hide: &[&str]) -> Vec<String> {
        let (active, hide) = (strings(active), strings(hide));
        ids(&visible_items(
            list,
            Filter {
                query,
                active: &active,
                hide: &hide,
            },
        ))
    }

    #[test]
    fn toggles_labels_but_unlabeled_goes_alone() {
        let t = |active: &[&str], label| toggle_label(&strings(active), label);
        assert_eq!(t(&["fav"], "r15"), strings(&["fav", "r15"]));
        assert_eq!(t(&["fav", "r15"], "fav"), strings(&["r15"]));
        // Another label takes ラベル非登録 off, and ラベル非登録 takes the others off.
        assert_eq!(t(&[UNLABELED], "fav"), strings(&["fav"]));
        assert_eq!(t(&["fav", "r15"], UNLABELED), strings(&[UNLABELED]));
        assert_eq!(t(&[UNLABELED], UNLABELED), Vec::<String>::new());
    }

    #[test]
    fn lists_everything_but_the_hidden_labels_without_a_query_or_label() {
        assert_eq!(
            find(&items(), "", &[], &["outdate"]),
            strings(&["1", "2", "3"])
        );
        assert_eq!(
            find(&items(), "", &[], &[]),
            strings(&["1", "2", "3", "4", "5"])
        );
    }

    #[test]
    fn finds_scattered_letters_after_the_plain_matches_like_fzf() {
        let games = vec![
            item("a", "Slay the Spire", &[]),
            item("b", "Street Fighter 6", &[]),
            item("c", "催眠アプリで彼女に完全堕ち", &[]),
            item("d", "Spire", &[]),
        ];
        assert_eq!(find(&games, "sts", &[], &[]), strings(&["a"]));
        assert_eq!(find(&games, "stf", &[], &[]), strings(&["b"]));
        assert_eq!(find(&games, "spire", &[], &[]), strings(&["a", "d"]));
        assert_eq!(find(&games, "sre", &[], &[]), strings(&["a", "b", "d"]));
        assert_eq!(find(&games, "催アプ", &[], &[]), strings(&["c"]));
        assert_eq!(find(&games, "zz", &[], &[]), Vec::<String>::new());
    }

    #[test]
    fn matches_the_maker_as_well_as_the_name() {
        let with = |id: &str, name: &str, detail: &str| Item {
            detail: Some(detail.into()),
            ..item(id, name, &[])
        };
        let games = vec![
            with("a", "夏のマゾ", "3Djp_Art"),
            with("b", "Other", "Somebody"),
        ];
        assert_eq!(find(&games, "3djp", &[], &[]), strings(&["a"]));
        assert_eq!(find(&games, "3djp 夏", &[], &[]), strings(&["a"]));
        assert_eq!(find(&games, "3djp other", &[], &[]), Vec::<String>::new());
    }

    #[test]
    fn matches_every_word_ignoring_case_and_width() {
        assert_eq!(find(&items(), "spire", &[], &[]), strings(&["2", "3"]));
        assert_eq!(find(&items(), "spire 2", &[], &[]), strings(&["3"]));
        assert_eq!(
            find(&items(), "  SLAY   spire ", &[], &[]),
            strings(&["2", "3"])
        );
        assert_eq!(find(&items(), "remnant", &[], &[]), strings(&["5"]));
        assert_eq!(find(&items(), "ｓｔｒｅｅｔ", &[], &[]), strings(&["1"]));
        assert_eq!(find(&items(), "zelda", &[], &[]), Vec::<String>::new());
    }

    #[test]
    fn keeps_games_in_any_selected_label() {
        assert_eq!(
            find(&items(), "", &["お気に入り", "R15"], &[]),
            strings(&["2", "3", "5"])
        );
        assert_eq!(find(&items(), "spire", &["R15"], &[]), strings(&["3"]));
    }

    #[test]
    fn unlabeled_keeps_the_games_without_labels() {
        assert_eq!(
            find(&items(), "", &[UNLABELED], &["outdate"]),
            strings(&["1"])
        );
        assert_eq!(
            find(&items(), "", &[UNLABELED, "お気に入り"], &[]),
            strings(&["1", "2"])
        );
        assert_eq!(
            find(&items(), "slay", &[UNLABELED], &[]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn shows_a_hidden_labels_games_only_while_it_is_selected() {
        let hide = &["outdate"];
        assert_eq!(find(&items(), "", &["R15"], hide), strings(&["3"]));
        assert_eq!(find(&items(), "", &["outdate"], hide), strings(&["4", "5"]));
        assert_eq!(
            find(&items(), "", &["R15", "outdate"], hide),
            strings(&["3", "4", "5"])
        );
        assert_eq!(find(&items(), "metal", &[], hide), Vec::<String>::new());
    }

    #[test]
    fn says_why_only_installed_games_are_listed() {
        let r = |reason| partial_reason(Some(reason)).unwrap();
        assert_eq!(partial_reason(None), None);
        assert!(r("secrets.yaml has no steam api_key").contains("API キー"));
        assert!(r("the Steam Web API rejected the key").contains("受け付けません"));
        assert!(r("the owned games are not fetched yet").contains("取得中"));
        assert!(
            r("the Steam Web API cannot be reached: timeout")
                .contains("the Steam Web API cannot be reached: timeout")
        );
        assert_eq!(
            r(r"D:\DLsiteNest\Game does not exist"),
            r"ゲームのフォルダ D:\DLsiteNest\Game がありません"
        );
        assert_eq!(
            r(r"there are no games in D:\Games"),
            r"D:\Games にゲームがありません"
        );
    }

    #[test]
    fn says_why_labels_cannot_be_changed() {
        assert_eq!(labels_locked_reason(None), None);
        assert_eq!(
            labels_locked_reason(Some("Steam is not running")).unwrap(),
            "Steam が起動していないため、ラベルは変更できません"
        );
        assert_eq!(
            labels_locked_reason(Some("Steam does not accept remote control: create .cef-enable-remote-debugging in the Steam folder and restart Steam")).unwrap(),
            "Steam の操作口が無効なため、ラベルは変更できません（Steam フォルダに .cef-enable-remote-debugging を置いて Steam を再起動）"
        );
        assert_eq!(
            labels_locked_reason(Some("timeout")).unwrap(),
            "ラベルを変更できません: timeout"
        );
    }

    #[test]
    fn asks_windows_link_for_a_games_picture() {
        assert_eq!(
            picture_url("http://127.0.0.1:4730", "games", "1364780"),
            "http://127.0.0.1:4730/buttons/games/library/1364780/image"
        );
        assert_eq!(
            picture_url("http://h", "a b", "x/y"),
            "http://h/buttons/a%20b/library/x%2Fy/image"
        );
        assert_eq!(
            encode("ゲーム's (1)!"),
            "%E3%82%B2%E3%83%BC%E3%83%A0's%20(1)!"
        );
    }

    #[test]
    fn lists_the_pins_of_the_library_buttons_in_button_order() {
        let pin = |id: &str| Pin {
            id: id.into(),
            name: id.into(),
        };
        let library = |id: &str, pins: Vec<Pin>| Button {
            id: id.into(),
            label: id.into(),
            desktop: None,
            except: vec![],
            icon: false,
            state: State::Library(LibraryState {
                pins,
                ..LibraryState::default()
            }),
        };
        let output = Button {
            state: State::Unknown,
            ..library("output", vec![])
        };
        let buttons = [
            library("games", vec![pin("1"), pin("2")]),
            output,
            library("more", vec![pin("3")]),
        ];
        let pinned = |button: &str, id: &str| Pinned {
            button: button.into(),
            pin: pin(id),
        };
        assert_eq!(
            pins_of(&buttons),
            [
                pinned("games", "1"),
                pinned("games", "2"),
                pinned("more", "3")
            ]
        );
    }

    fn dated(id: &str, updated: Option<i64>, bought: Option<i64>) -> Item {
        Item {
            updated,
            bought,
            ..item(id, id, &[])
        }
    }

    #[test]
    fn keeps_windows_links_order_or_puts_the_newest_date_first() {
        let list = [
            dated("1", Some(300), None),
            dated("2", None, None),
            dated("3", Some(500), Some(10)),
            dated("4", Some(100), Some(20)),
        ];
        assert_eq!(
            ids(&sort_items(&list, Order::Recent)),
            strings(&["1", "2", "3", "4"])
        );
        assert_eq!(
            ids(&sort_items(&list, Order::Updated)),
            strings(&["3", "1", "4", "2"])
        );
        // Games without the date keep windows-link's order among themselves.
        assert_eq!(
            ids(&sort_items(&list, Order::Bought)),
            strings(&["4", "3", "1", "2"])
        );
        assert_eq!(
            orders_of(&list),
            [Order::Recent, Order::Updated, Order::Bought]
        );
        assert_eq!(
            orders_of(&[dated("1", Some(300), None)]),
            [Order::Recent, Order::Updated]
        );
        // Steam's games have no dates.
        assert_eq!(orders_of(&items()), [Order::Recent]);
    }

    #[test]
    fn writes_a_date_as_the_day_it_was() {
        // 2026-10-15 15:00 UTC is the 16th in Japan.
        assert_eq!(
            date_label(1_792_076_400, &FixedOffset::east_opt(9 * 3600).unwrap()),
            "2026/10/16"
        );
        assert_eq!(date_label(1_792_076_400, &Utc), "2026/10/15");
    }
}
