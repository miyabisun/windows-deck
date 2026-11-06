//! What a button shows for a windows-link state, and why a request failed, in words the user
//! can act on.

use chrono::{DateTime, TimeZone};
use serde_json::Value;

use super::library::{labels_locked_reason, steam_down};
use super::protocol::{ErrorBody, Round, State};

/// What a button says: the large state, the note under it, and whether it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub text: String,
    pub note: Option<String>,
    pub failed: bool,
}

impl Shown {
    fn new(text: impl Into<String>, note: Option<String>) -> Self {
        Self {
            text: text.into(),
            note,
            failed: false,
        }
    }
}

/// The line icons the panel draws (24×24, `currentColor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Glyph {
    Plus,
    Moon,
    Sun,
    Close,
    Check,
    /// A speaker cone alone, for the mixer.
    Volume,
    Sound,
    Muted,
    Speaker,
    Headphones,
}

/// The colours a highlighted cursor comes in, as the menu lists them.
const COLORS: [(&str, &str); 3] = [("red", "赤"), ("blue", "青"), ("green", "緑")];

/// A colour to choose for a highlighted cursor, as the `cursor.highlight` button's menu
/// lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorChoice {
    pub color: &'static str,
    pub label: String,
}

/// The colours to choose for a highlighted cursor, the chosen one marked.
pub fn color_menu(state: &State) -> Vec<ColorChoice> {
    let chosen = match state {
        State::Highlight { color, .. } => color.as_deref(),
        _ => None,
    };
    COLORS
        .iter()
        .map(|&(color, name)| ColorChoice {
            color,
            label: if chosen == Some(color) {
                format!("{name}（選択中）")
            } else {
                name.to_owned()
            },
        })
        .collect()
}

fn percent(volume: f64) -> i64 {
    // Volumes are 0-1, so the rounded percentage always fits.
    #[allow(clippy::cast_possible_truncation)]
    let rounded = (volume * 100.0).round() as i64;
    rounded
}

pub fn describe_state(state: &State) -> Shown {
    match state {
        State::Output {
            current,
            current_name,
            options,
        } => {
            let others: Vec<String> = options
                .iter()
                .filter(|o| Some(&o.alias) != current.as_ref() && !o.connected)
                .map(|o| format!("{} は未接続", o.name.as_deref().unwrap_or(&o.alias)))
                .collect();
            let text = current_name
                .as_deref()
                .or(current.as_deref())
                .unwrap_or("出力先が不明");
            Shown::new(text, (!others.is_empty()).then(|| others.join("、")))
        }
        State::Volume { running, volume } => {
            let text = match (running, volume) {
                (true, Some(volume)) => format!("{}%", percent(*volume)),
                (true, None) => "0%".into(),
                (false, _) => "起動していません".into(),
            };
            Shown::new(text, None)
        }
        State::Launch { running: true } => Shown::new("前に出す", Some("起動中".into())),
        State::Game { running: true } => Shown::new("終了", Some("起動中".into())),
        State::Launch { running: false } | State::Game { running: false } => {
            Shown::new("起動", None)
        }
        State::Library(library) => Shown::new(
            "ライブラリ",
            library
                .sign_in
                .is_some()
                .then(|| "ログインが必要です".into()),
        ),
        State::AppMute { running: false, .. } => Shown::new("起動していません", None),
        State::Mute { muted: true, .. } | State::AppMute { muted: true, .. } => {
            Shown::new("ミュート解除", Some("ミュート中".into()))
        }
        State::Mute {
            muted: false,
            volume,
        } => Shown::new("ミュート", Some(format!("音量 {}%", percent(*volume)))),
        State::AppMute { muted: false, .. } => Shown::new("ミュート", None),
        State::Mixer { muted, volume } => Shown::new(
            format!("音量 {}%", percent(*volume)),
            muted.then(|| "ミュート中".into()),
        ),
        State::Hold { held: true } => Shown::new("自動操作を再開", Some("ユーザー操作中".into())),
        State::Hold { held: false } => Shown::new("ユーザー操作中にする", None),
        State::Highlight { on: true, color } => {
            let name = COLORS
                .iter()
                .find(|(c, _)| Some(*c) == color.as_deref())
                .map(|(_, name)| *name);
            let note = name.map_or_else(
                || "目立たせ中".into(),
                |name| format!("目立たせ中（{name}）"),
            );
            Shown::new("元に戻す", Some(note))
        }
        State::Highlight { on: false, .. } => Shown::new("目立たせる", None),
        State::Error { message } => Shown {
            text: "状態を読めません".into(),
            note: message.clone(),
            failed: true,
        },
        State::Unknown => Shown::new("状態不明", None),
    }
}

/// The icon of a button's state, if it has one: the current output device (when windows-link
/// says what each is), sound or muted, and a speaker for the mixer.
pub fn state_icon(state: &State) -> Option<Glyph> {
    match state {
        State::Output {
            current, options, ..
        } => {
            let current = current.as_ref()?;
            match options
                .iter()
                .find(|o| &o.alias == current)?
                .icon
                .as_deref()?
            {
                "speaker" => Some(Glyph::Speaker),
                "headphones" => Some(Glyph::Headphones),
                _ => None,
            }
        }
        State::Mute { muted, .. } | State::AppMute { muted, .. } => {
            Some(if *muted { Glyph::Muted } else { Glyph::Sound })
        }
        State::Mixer { .. } => Some(Glyph::Volume),
        _ => None,
    }
}

/// An HTTP status, or none when the request got no answer.
pub type Status = Option<u16>;

/// The reason shown on a button after a failed press.
pub fn press_failure(status: Status, body: Option<&ErrorBody>) -> String {
    let Some(status) = status else {
        return "windows-link に届きませんでした".into();
    };
    let error = body.and_then(|b| b.error.as_deref());
    let message = body.and_then(|b| b.message.as_deref());
    match error {
        Some("device_unavailable") => return "切替先のデバイスがつながっていません".into(),
        Some("not_running") => return "対象のアプリが起動していません".into(),
        Some("not_found") => {
            return "windows-link にこのボタンがありません（設定が変わった可能性があります）"
                .into();
        }
        Some("no_window") => return "ゲームのウィンドウがまだ無いため閉じられません".into(),
        Some("launch") => {
            return match message {
                Some(message) => format!("起動できませんでした: {message}"),
                None => "起動できませんでした".into(),
            };
        }
        _ => {}
    }
    match message {
        Some(message) if !message.is_empty() => {
            format!("Windows の操作に失敗しました: {message}")
        }
        _ => format!("windows-link がエラーを返しました（HTTP {status}）"),
    }
}

/// Why resting or waking did not go through: the panel's monitor unknown (`no_monitor`, from
/// the panel itself), or windows-link refusing or failing.
pub fn rest_failure(status: Status, body: Option<&ErrorBody>) -> String {
    match body.and_then(|b| b.error.as_deref()) {
        Some("no_monitor") => {
            "deck のモニターが分からないため休止できません（設定の monitor を確かめてください）"
                .into()
        }
        Some("not_found") => "deck のモニターが windows-link に見つかりません".into(),
        Some("unrestored") => format!(
            "明るさを戻せなかったモニターがあります。そのモニターのボタンで戻してください（{}）",
            body.map(ErrorBody::message).unwrap_or_default()
        ),
        _ => press_failure(status, body),
    }
}

/// Why signing in to a shop did not go through: the login window (no status, with `closed`,
/// or `not_panel` outside the panel), or windows-link refusing it.
pub fn sign_in_failure(status: Status, body: Option<&ErrorBody>) -> String {
    match body.and_then(|b| b.error.as_deref()) {
        Some("closed") => "ログインせずにログイン画面が閉じられました".into(),
        Some("not_panel") => "ログインはタッチパネルのアプリからだけできます".into(),
        Some("invalid_session") => {
            "ログインを確認できませんでした。もう一度ログインしてください".into()
        }
        _ => press_failure(status, body),
    }
}

/// The reason shown after a library game could not be started, pinned or unpinned.
pub fn library_failure(status: Status, body: Option<&ErrorBody>) -> String {
    match body.and_then(|b| b.error.as_deref()) {
        Some("no_program") => {
            "起動できるファイルが見つかりません（ローカルファイル閲覧で中を確かめてください）"
                .into()
        }
        Some("not_found") => {
            "ライブラリにこのゲームがありません（一覧を開き直してください）".into()
        }
        Some("not_downloaded") => format!(
            "まだこの PC にありません（{}）",
            body.map(ErrorBody::message).unwrap_or_default()
        ),
        _ => press_failure(status, body),
    }
}

/// What updating a library started: Steam's updates and the installs it asks to confirm on its
/// own window, or a shop's round of downloads.
pub fn update_result(body: Option<&Value>) -> String {
    let count = |name: &str| body.and_then(|b| b.get(name)).and_then(Value::as_f64);
    let Some(updates) = count("updates") else {
        return "ダウンロードと更新を始めました（進み具合はライブラリに出ます）".into();
    };
    let installs = count("installs").unwrap_or(0.0);
    let mut said = Vec::new();
    if updates > 0.0 {
        said.push(format!("アップデート {updates} 件を始めました"));
    }
    if installs > 0.0 {
        said.push(format!(
            "インストール {installs} 件は Steam の画面で確定してください"
        ));
    }
    if said.is_empty() {
        "アップデート・インストールするゲームはありません".into()
    } else {
        said.join("。")
    }
}

/// Why a shop's round could not read the purchases, in words the user can act on.
fn purchases_reason(reason: &str) -> String {
    if reason.contains("no DLsite account") {
        return "windows-link の secrets.yaml に DLsite のアカウントがありません".into();
    }
    if let Some(rest) = reason
        .split_once("login page: http status: ")
        .map(|(_, r)| r)
    {
        let code: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if !code.is_empty() {
            return format!("DLsite のログイン画面が HTTP {code} を返しました");
        }
    }
    if let Some((shop, _)) = reason.split_once(" needs signing in again through the panel")
        && !shop.is_empty()
        && !shop.contains(char::is_whitespace)
    {
        return format!("{shop} へのログインが必要です。ライブラリを開いてログインしてください");
    }
    reason.to_owned()
}

/// What a shop's round is doing, or how the last one ended, and why it could not read the
/// purchases or the games' folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundSummary {
    pub busy: bool,
    pub text: String,
    pub failure: Option<String>,
}

pub fn round_summary<Tz: TimeZone>(round: &Round, zone: &Tz) -> RoundSummary
where
    Tz::Offset: std::fmt::Display,
{
    let failure = if let Some(purchases) = &round.purchases {
        Some(format!(
            "購入一覧を読めませんでした（{}）。新しいゲームと更新は探せていません",
            purchases_reason(purchases)
        ))
    } else {
        round.folder.as_ref().map(|folder| {
            format!("ゲームのフォルダを読めないため、ダウンロードしませんでした（{folder}）")
        })
    };
    let jobs = &round.jobs;
    let done = jobs.iter().filter(|j| j.status == "完了").count();
    let busy = |text: String| RoundSummary {
        busy: true,
        text,
        failure: failure.clone(),
    };
    match round.doing.as_deref() {
        Some("purchases") => return busy("購入一覧を読んでいます".into()),
        Some("downloads") => {
            return busy(format!(
                "ダウンロードと更新: {} 件のうち {done} 件が完了",
                jobs.len()
            ));
        }
        _ => {}
    }
    let Some(ended) = round.finished.and_then(|at| local(at, zone)) else {
        return RoundSummary {
            busy: false,
            text: "windows-link の起動から、まだ一度も更新していません".into(),
            failure,
        };
    };
    let failed = jobs.iter().filter(|j| j.status.contains("失敗")).count();
    // Without the purchases it could not look, so "nothing new" would not be true.
    let result = if !jobs.is_empty() {
        Some(format!("完了 {done} 件・失敗 {failed} 件"))
    } else if failure.is_some() {
        None
    } else {
        Some("新しいゲームも更新もありませんでした".into())
    };
    let ended = ended.format("%m/%d %H:%M");
    RoundSummary {
        busy: false,
        text: match result {
            Some(result) => format!("{ended} に終わりました。{result}"),
            None => format!("{ended} に終わりました"),
        },
        failure,
    }
}

/// Unix seconds as a time in `zone`.
pub fn local<Tz: TimeZone>(seconds: i64, zone: &Tz) -> Option<DateTime<Tz>> {
    zone.timestamp_opt(seconds, 0).single()
}

/// The reason shown on a library button after it could not be updated.
pub fn update_failure(status: Status, body: Option<&ErrorBody>) -> String {
    match body.and_then(|b| b.error.as_deref()) {
        Some("sign_in") => "ログインが必要です（ライブラリを開いてログインしてください）".into(),
        Some("update_unavailable") => {
            let message = body.map(ErrorBody::message).unwrap_or_default();
            steam_down(message, "ライブラリを更新できません")
                .unwrap_or_else(|| format!("ライブラリを更新できません: {message}"))
        }
        _ => press_failure(status, body),
    }
}

/// The reason shown when the mixer cannot read or change a volume.
pub fn mixer_failure(status: Status, body: Option<&ErrorBody>) -> String {
    match body.and_then(|b| b.error.as_deref()) {
        Some("not_found") => {
            "このアプリは今は音を出していません（ミキサーを開き直してください）".into()
        }
        Some("audio") => format!(
            "音量を操作できません: {}",
            body.map(ErrorBody::message).unwrap_or_default()
        ),
        _ => press_failure(status, body),
    }
}

/// The reason shown when a game's license keys cannot be shown.
pub fn keys_failure(status: Status, body: Option<&ErrorBody>) -> String {
    match body.and_then(|b| b.error.as_deref()) {
        Some("keys_unavailable") => format!(
            "DLsite に問い合わせできません: {}",
            body.map(ErrorBody::message).unwrap_or_default()
        ),
        Some("not_found") => {
            "このゲームのシリアル番号は分かりません（DLsite の作品が分かっていません）".into()
        }
        _ => press_failure(status, body),
    }
}

/// The reason shown after a label could not be created, renamed, deleted or filled.
pub fn label_failure(status: Status, body: Option<&ErrorBody>) -> String {
    let message = body.map(ErrorBody::message).unwrap_or_default();
    match body.and_then(|b| b.error.as_deref()) {
        Some("not_found") => "ラベルかゲームが見つかりません（一覧を開き直してください）".into(),
        Some("labels_unavailable") => labels_locked_reason(Some(message)).unwrap_or_default(),
        Some("invalid_label" | "labels") => format!("ラベルを変更できません: {message}"),
        _ => press_failure(status, body),
    }
}

/// The reason shown after a desktop tab could not switch Windows to its desktop.
pub fn switch_failure(status: Status, body: Option<&ErrorBody>) -> String {
    let Some(status) = status else {
        return "windows-link に届きませんでした".into();
    };
    if body.is_some_and(|b| b.is("not_found")) {
        return "このデスクトップはもうありません".into();
    }
    let message = body
        .and_then(|b| b.message.clone())
        .unwrap_or_else(|| format!("HTTP {status}"));
    format!("仮想デスクトップを切り替えられません: {message}")
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, Utc};
    use serde_json::json;

    use super::*;
    use crate::model::protocol::{Job, LibraryState, OutputOption};

    fn state(value: serde_json::Value) -> State {
        serde_json::from_value(value).unwrap()
    }

    fn shown(text: &str, note: Option<&str>) -> Shown {
        Shown {
            text: text.into(),
            note: note.map(Into::into),
            failed: false,
        }
    }

    fn body(error: &str, message: Option<&str>) -> ErrorBody {
        ErrorBody {
            error: Some(error.into()),
            message: message.map(Into::into),
        }
    }

    #[test]
    fn mute_and_mixer_say_what_a_press_does_and_the_volume() {
        let d = |v| describe_state(&state(v));
        assert_eq!(
            d(json!({"kind":"mute","muted":false,"volume":0.45})),
            shown("ミュート", Some("音量 45%"))
        );
        assert_eq!(
            d(json!({"kind":"mute","muted":true,"volume":0.45})),
            shown("ミュート解除", Some("ミュート中"))
        );
        assert_eq!(
            d(json!({"kind":"mixer","muted":false,"volume":0.45})),
            shown("音量 45%", None)
        );
        assert_eq!(
            d(json!({"kind":"mixer","muted":true,"volume":0.45})),
            shown("音量 45%", Some("ミュート中"))
        );
    }

    #[test]
    fn app_mute_says_what_a_press_does_and_when_it_has_no_sound() {
        let d = |v| describe_state(&state(v));
        assert_eq!(
            d(json!({"kind":"app_mute","running":true,"muted":false})),
            shown("ミュート", None)
        );
        assert_eq!(
            d(json!({"kind":"app_mute","running":true,"muted":true})),
            shown("ミュート解除", Some("ミュート中"))
        );
        assert_eq!(
            d(json!({"kind":"app_mute","running":false,"muted":false})),
            shown("起動していません", None)
        );
    }

    #[test]
    fn state_icon_shows_the_device_the_mute_and_the_mixer() {
        let output = |current: Option<&str>, icons: Option<[&str; 2]>| State::Output {
            current: current.map(Into::into),
            current_name: None,
            options: ["d3v", "jbl"]
                .iter()
                .enumerate()
                .map(|(i, alias)| OutputOption {
                    alias: (*alias).into(),
                    name: None,
                    connected: true,
                    icon: icons.map(|icons| icons[i].into()),
                })
                .collect(),
        };
        let both = Some(["speaker", "headphones"]);
        assert_eq!(state_icon(&output(Some("d3v"), both)), Some(Glyph::Speaker));
        assert_eq!(
            state_icon(&output(Some("jbl"), both)),
            Some(Glyph::Headphones)
        );
        assert_eq!(state_icon(&output(Some("jbl"), None)), None);
        assert_eq!(state_icon(&output(None, both)), None);
        assert_eq!(
            state_icon(&state(json!({"kind":"mute","muted":false,"volume":1}))),
            Some(Glyph::Sound)
        );
        assert_eq!(
            state_icon(&state(json!({"kind":"mute","muted":true,"volume":1}))),
            Some(Glyph::Muted)
        );
        assert_eq!(
            state_icon(&state(
                json!({"kind":"app_mute","running":true,"muted":false})
            )),
            Some(Glyph::Sound)
        );
        assert_eq!(
            state_icon(&state(
                json!({"kind":"app_mute","running":true,"muted":true})
            )),
            Some(Glyph::Muted)
        );
        assert_eq!(
            state_icon(&state(json!({"kind":"mixer","muted":false,"volume":1}))),
            Some(Glyph::Volume)
        );
        assert_eq!(
            state_icon(&state(json!({"kind":"game","running":false}))),
            None
        );
        assert_eq!(state_icon(&State::Unknown), None);
    }

    #[test]
    fn mixer_failure_says_why_a_volume_cannot_change() {
        assert_eq!(
            mixer_failure(Some(404), Some(&ErrorBody::named("not_found"))),
            "このアプリは今は音を出していません（ミキサーを開き直してください）"
        );
        assert_eq!(
            mixer_failure(Some(500), Some(&body("audio", Some("no device")))),
            "音量を操作できません: no device"
        );
        assert_eq!(mixer_failure(None, None), "windows-link に届きませんでした");
    }

    #[test]
    fn shows_the_output_and_notes_a_disconnected_alternative() {
        let s = state(
            json!({"kind":"output","current":"motu","current_name":"MOTU (MOTU M Series)","options":[
            {"alias":"motu","name":"MOTU (MOTU M Series)","connected":true},
            {"alias":"jbl","name":"BTイヤホン (JBL Tour Pro 3)","connected":false}]}),
        );
        assert_eq!(
            describe_state(&s),
            shown(
                "MOTU (MOTU M Series)",
                Some("BTイヤホン (JBL Tour Pro 3) は未接続")
            )
        );
        let options = json!([{"alias":"jbl","name":"JBL","connected":true}]);
        let text = |current: serde_json::Value| {
            describe_state(&state(
                json!({"kind":"output","current":current,"current_name":null,"options":options}),
            ))
            .text
        };
        assert_eq!(text(json!("jbl")), "jbl");
        assert_eq!(text(json!(null)), "出力先が不明");
    }

    #[test]
    fn shows_an_apps_volume_or_that_it_is_not_running() {
        let d = |v| describe_state(&state(v));
        assert_eq!(
            d(json!({"kind":"volume","running":true,"volume":0.2,"levels":[0.2,1]})),
            shown("20%", None)
        );
        assert_eq!(
            d(json!({"kind":"volume","running":true,"volume":0.999})).text,
            "100%"
        );
        assert_eq!(
            d(json!({"kind":"volume","running":false,"volume":null})).text,
            "起動していません"
        );
    }

    #[test]
    fn a_state_windows_link_cannot_read_fails_with_its_message() {
        assert_eq!(
            describe_state(&state(
                json!({"kind":"error","message":"audio service stopped"})
            )),
            Shown {
                text: "状態を読めません".into(),
                note: Some("audio service stopped".into()),
                failed: true
            }
        );
    }

    #[test]
    fn hold_and_highlight_say_what_a_press_does() {
        let d = |v| describe_state(&state(v));
        assert_eq!(
            d(json!({"kind":"hold","held":false})),
            shown("ユーザー操作中にする", None)
        );
        assert_eq!(
            d(json!({"kind":"hold","held":true})),
            shown("自動操作を再開", Some("ユーザー操作中"))
        );
        assert_eq!(
            d(json!({"kind":"highlight","on":false,"color":"blue"})),
            shown("目立たせる", None)
        );
        assert_eq!(
            d(json!({"kind":"highlight","on":true,"color":"red"})),
            shown("元に戻す", Some("目立たせ中（赤）"))
        );
        // An older windows-link tells no colour.
        assert_eq!(
            d(json!({"kind":"highlight","on":true})),
            shown("元に戻す", Some("目立たせ中"))
        );
    }

    #[test]
    fn lists_the_cursor_colours_the_chosen_one_marked() {
        let menu = color_menu(&state(
            json!({"kind":"highlight","on":false,"color":"blue"}),
        ));
        let pairs: Vec<(&str, &str)> = menu.iter().map(|c| (c.color, c.label.as_str())).collect();
        assert_eq!(
            pairs,
            [("red", "赤"), ("blue", "青（選択中）"), ("green", "緑")]
        );
    }

    #[test]
    fn an_unknown_state_does_not_break() {
        assert_eq!(
            describe_state(&state(json!({"kind":"future"}))),
            shown("状態不明", None)
        );
        assert_eq!(describe_state(&State::Unknown), shown("状態不明", None));
    }

    #[test]
    fn press_failure_explains_what_a_user_can_fix() {
        assert_eq!(
            press_failure(Some(409), Some(&ErrorBody::named("device_unavailable"))),
            "切替先のデバイスがつながっていません"
        );
        assert_eq!(
            press_failure(Some(409), Some(&ErrorBody::named("not_running"))),
            "対象のアプリが起動していません"
        );
        assert_eq!(
            press_failure(Some(404), Some(&ErrorBody::named("not_found"))),
            "windows-link にこのボタンがありません（設定が変わった可能性があります）"
        );
        assert_eq!(
            press_failure(Some(500), Some(&body("audio", Some("COM failed")))),
            "Windows の操作に失敗しました: COM failed"
        );
        assert_eq!(
            press_failure(Some(502), None),
            "windows-link がエラーを返しました（HTTP 502）"
        );
        assert_eq!(press_failure(None, None), "windows-link に届きませんでした");
        assert_eq!(
            press_failure(Some(409), Some(&body("no_window", Some("x")))),
            "ゲームのウィンドウがまだ無いため閉じられません"
        );
        assert_eq!(
            press_failure(Some(500), Some(&body("launch", Some("cannot open x")))),
            "起動できませんでした: cannot open x"
        );
    }

    #[test]
    fn switch_failure_explains_a_desktop_switch() {
        assert_eq!(
            switch_failure(Some(404), Some(&ErrorBody::named("not_found"))),
            "このデスクトップはもうありません"
        );
        assert_eq!(
            switch_failure(Some(503), Some(&body("desktops", Some("service stopped")))),
            "仮想デスクトップを切り替えられません: service stopped"
        );
        assert_eq!(
            switch_failure(None, None),
            "windows-link に届きませんでした"
        );
    }

    #[test]
    fn a_library_button_opens_the_list_and_says_when_to_sign_in() {
        let library = |sign_in: Option<&str>| {
            State::Library(LibraryState {
                sign_in: sign_in.map(Into::into),
                ..LibraryState::default()
            })
        };
        assert_eq!(
            describe_state(&library(Some("fanza"))),
            shown("ライブラリ", Some("ログインが必要です"))
        );
        assert_eq!(describe_state(&library(None)), shown("ライブラリ", None));
    }

    #[test]
    fn explains_a_sign_in_that_did_not_go_through() {
        assert_eq!(
            sign_in_failure(None, Some(&ErrorBody::named("closed"))),
            "ログインせずにログイン画面が閉じられました"
        );
        assert_eq!(
            sign_in_failure(None, Some(&ErrorBody::named("not_panel"))),
            "ログインはタッチパネルのアプリからだけできます"
        );
        assert_eq!(
            sign_in_failure(Some(400), Some(&body("invalid_session", Some("x")))),
            "ログインを確認できませんでした。もう一度ログインしてください"
        );
        assert_eq!(
            sign_in_failure(None, None),
            "windows-link に届きませんでした"
        );
    }

    #[test]
    fn launch_and_game_buttons_say_what_a_press_will_do() {
        let d = |v| describe_state(&state(v));
        assert_eq!(
            d(json!({"kind":"launch","running":false})),
            shown("起動", None)
        );
        assert_eq!(
            d(json!({"kind":"launch","running":true})),
            shown("前に出す", Some("起動中"))
        );
        assert_eq!(
            d(json!({"kind":"game","running":false})),
            shown("起動", None)
        );
        assert_eq!(
            d(json!({"kind":"game","running":true})),
            shown("終了", Some("起動中"))
        );
    }

    #[test]
    fn explains_keys_starts_and_pins_that_failed() {
        assert_eq!(
            keys_failure(
                Some(409),
                Some(&body("keys_unavailable", Some("DLsite is down")))
            ),
            "DLsite に問い合わせできません: DLsite is down"
        );
        assert_eq!(
            keys_failure(Some(404), Some(&ErrorBody::named("not_found"))),
            "このゲームのシリアル番号は分かりません（DLsite の作品が分かっていません）"
        );
        assert_eq!(keys_failure(None, None), "windows-link に届きませんでした");
        assert_eq!(
            library_failure(Some(404), Some(&ErrorBody::named("not_found"))),
            "ライブラリにこのゲームがありません（一覧を開き直してください）"
        );
        assert_eq!(
            library_failure(
                Some(409),
                Some(&body("not_downloaded", Some("ダウンロード中 40%")))
            ),
            "まだこの PC にありません（ダウンロード中 40%）"
        );
        assert_eq!(
            library_failure(Some(500), Some(&body("launch", Some("denied")))),
            "起動できませんでした: denied"
        );
        assert_eq!(
            library_failure(None, None),
            "windows-link に届きませんでした"
        );
        assert_eq!(
            library_failure(Some(409), Some(&ErrorBody::named("no_program"))),
            "起動できるファイルが見つかりません（ローカルファイル閲覧で中を確かめてください）"
        );
    }

    #[test]
    fn says_what_updating_a_library_started() {
        let r = |v: serde_json::Value| update_result(Some(&v));
        assert_eq!(
            r(json!({"updates":3,"installs":24})),
            "アップデート 3 件を始めました。インストール 24 件は Steam の画面で確定してください"
        );
        assert_eq!(
            r(json!({"updates":1,"installs":0})),
            "アップデート 1 件を始めました"
        );
        assert_eq!(
            r(json!({"updates":0,"installs":2})),
            "インストール 2 件は Steam の画面で確定してください"
        );
        assert_eq!(
            r(json!({"updates":0,"installs":0})),
            "アップデート・インストールするゲームはありません"
        );
        assert_eq!(
            r(json!({"round":true})),
            "ダウンロードと更新を始めました（進み具合はライブラリに出ます）"
        );
        assert_eq!(
            update_result(None),
            "ダウンロードと更新を始めました（進み具合はライブラリに出ます）"
        );
    }

    #[test]
    fn explains_why_a_library_could_not_be_updated() {
        assert_eq!(
            update_failure(Some(409), Some(&body("sign_in", Some("x")))),
            "ログインが必要です（ライブラリを開いてログインしてください）"
        );
        assert_eq!(
            update_failure(
                Some(409),
                Some(&body("update_unavailable", Some("Steam is not running")))
            ),
            "Steam が起動していないため、ライブラリを更新できません"
        );
        assert_eq!(
            update_failure(
                Some(409),
                Some(&body(
                    "update_unavailable",
                    Some(
                        "Steam does not accept remote control: create .cef-enable-remote-debugging in the Steam folder and restart Steam"
                    )
                ))
            ),
            "Steam の操作口が無効なため、ライブラリを更新できません（Steam フォルダに .cef-enable-remote-debugging を置いて Steam を再起動）"
        );
        assert_eq!(
            update_failure(
                Some(409),
                Some(&body(
                    "update_unavailable",
                    Some("DLsite cannot download games now")
                ))
            ),
            "ライブラリを更新できません: DLsite cannot download games now"
        );
        assert_eq!(
            update_failure(Some(500), Some(&body("update", Some("TypeError: boom")))),
            "Windows の操作に失敗しました: TypeError: boom"
        );
        assert_eq!(
            update_failure(None, None),
            "windows-link に届きませんでした"
        );
    }

    #[test]
    fn says_why_a_label_could_not_be_changed() {
        assert_eq!(
            label_failure(
                Some(409),
                Some(&body(
                    "labels_unavailable",
                    Some("Steam が起動していません")
                ))
            ),
            "ラベルを変更できません: Steam が起動していません"
        );
        assert_eq!(
            label_failure(
                Some(400),
                Some(&body("invalid_label", Some("a label named \"RPG\" exists")))
            ),
            "ラベルを変更できません: a label named \"RPG\" exists"
        );
        assert_eq!(
            label_failure(Some(404), Some(&ErrorBody::named("not_found"))),
            "ラベルかゲームが見つかりません（一覧を開き直してください）"
        );
        assert_eq!(label_failure(None, None), "windows-link に届きませんでした");
    }

    #[test]
    fn says_why_resting_or_waking_did_not_go_through() {
        assert_eq!(rest_failure(None, None), "windows-link に届きませんでした");
        assert_eq!(
            rest_failure(None, Some(&ErrorBody::named("no_monitor"))),
            "deck のモニターが分からないため休止できません（設定の monitor を確かめてください）"
        );
        assert_eq!(
            rest_failure(Some(404), Some(&ErrorBody::named("not_found"))),
            "deck のモニターが windows-link に見つかりません"
        );
        assert_eq!(
            rest_failure(
                Some(409),
                Some(&body(
                    "unrestored",
                    Some("cannot put the brightness back on 25M2S")
                ))
            ),
            "明るさを戻せなかったモニターがあります。そのモニターのボタンで戻してください（cannot put the brightness back on 25M2S）"
        );
        assert_eq!(
            rest_failure(Some(500), Some(&body("rest", Some("store")))),
            "Windows の操作に失敗しました: store"
        );
    }

    fn job(id: &str, status: &str) -> Job {
        Job {
            id: id.into(),
            name: format!("{id} name"),
            status: status.into(),
        }
    }

    /// 2026-10-28 01:35:30 in Japan.
    const AT: i64 = 1_793_118_930;

    fn tokyo() -> FixedOffset {
        FixedOffset::east_opt(9 * 3600).unwrap()
    }

    #[test]
    fn says_what_the_round_under_way_does() {
        let reading = Round {
            doing: Some("purchases".into()),
            started: Some(AT),
            ..Round::default()
        };
        assert_eq!(
            round_summary(&reading, &tokyo()),
            RoundSummary {
                busy: true,
                text: "購入一覧を読んでいます".into(),
                failure: None
            }
        );
        let downloads = Round {
            doing: Some("downloads".into()),
            started: Some(AT),
            jobs: vec![
                job("RJ1", "完了"),
                job("RJ2", "ダウンロード中 40%"),
                job("RJ3", "更新待ち"),
            ],
            ..Round::default()
        };
        assert_eq!(
            round_summary(&downloads, &tokyo()),
            RoundSummary {
                busy: true,
                text: "ダウンロードと更新: 3 件のうち 1 件が完了".into(),
                failure: None
            }
        );
    }

    #[test]
    fn says_how_the_last_round_ended_and_why_it_could_not_read() {
        let ended = |fields: Round| Round {
            started: Some(AT - 60),
            finished: Some(AT),
            ..fields
        };
        assert_eq!(
            round_summary(&Round::default(), &tokyo()),
            RoundSummary {
                busy: false,
                text: "windows-link の起動から、まだ一度も更新していません".into(),
                failure: None
            }
        );
        assert_eq!(
            round_summary(&ended(Round::default()), &tokyo()),
            RoundSummary {
                busy: false,
                text: "10/28 01:35 に終わりました。新しいゲームも更新もありませんでした".into(),
                failure: None
            }
        );
        let some = ended(Round {
            jobs: vec![
                job("RJ1", "完了"),
                job("RJ2", "ダウンロードに失敗: disk full"),
            ],
            ..Round::default()
        });
        assert_eq!(
            round_summary(&some, &tokyo()).text,
            "10/28 01:35 に終わりました。完了 1 件・失敗 1 件"
        );
        let blocked = ended(Round {
            purchases: Some("DLsite's login page: http status: 403".into()),
            ..Round::default()
        });
        assert_eq!(
            round_summary(&blocked, &tokyo()).failure.as_deref(),
            Some(
                "購入一覧を読めませんでした（DLsite のログイン画面が HTTP 403 を返しました）。新しいゲームと更新は探せていません"
            )
        );
        let failure = |purchases: &str| {
            round_summary(
                &Round {
                    purchases: Some(purchases.into()),
                    ..Round::default()
                },
                &Utc,
            )
            .failure
            .unwrap()
        };
        assert_eq!(
            failure("secrets.yaml has no DLsite account"),
            "購入一覧を読めませんでした（windows-link の secrets.yaml に DLsite のアカウントがありません）。新しいゲームと更新は探せていません"
        );
        assert_eq!(
            failure("FANZA needs signing in again through the panel"),
            "購入一覧を読めませんでした（FANZA へのログインが必要です。ライブラリを開いてログインしてください）。新しいゲームと更新は探せていません"
        );
        assert_eq!(
            failure("DLsite needs signing in again through the panel"),
            "購入一覧を読めませんでした（DLsite へのログインが必要です。ライブラリを開いてログインしてください）。新しいゲームと更新は探せていません"
        );
        // A games folder that cannot be read keeps the round from downloading.
        let unreadable = ended(Round {
            folder: Some(
                r"D:\DLsiteNest\Game\maker: 存在しないデバイスを指定しました。 (os error 433)"
                    .into(),
            ),
            ..Round::default()
        });
        assert_eq!(
            round_summary(&unreadable, &tokyo()),
            RoundSummary {
                busy: false,
                text: "10/28 01:35 に終わりました".into(),
                failure: Some(r"ゲームのフォルダを読めないため、ダウンロードしませんでした（D:\DLsiteNest\Game\maker: 存在しないデバイスを指定しました。 (os error 433)）".into()),
            }
        );
    }
}
