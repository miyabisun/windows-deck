//! What windows-link sends: its buttons and their states, the virtual desktops, the libraries,
//! the mixer, and its error bodies. Fields a newer windows-link adds are ignored, and a state
//! this version cannot read shows as unknown instead of dropping the whole list.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Button {
    pub id: String,
    #[serde(default)]
    pub label: String,
    /// The desktop whose file the button comes from; none for a shared button.
    #[serde(default)]
    pub desktop: Option<String>,
    /// The desktops a shared button stays off.
    #[serde(default)]
    pub except: Vec<String>,
    /// Whether windows-link serves a picture for it (`GET /buttons/{id}/icon`).
    #[serde(default, deserialize_with = "truthy")]
    pub icon: bool,
    #[serde(default, deserialize_with = "lenient")]
    pub state: State,
}

/// A button's state, by its `kind`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum State {
    Output {
        #[serde(default)]
        current: Option<String>,
        #[serde(default)]
        current_name: Option<String>,
        #[serde(default)]
        options: Vec<OutputOption>,
    },
    Volume {
        #[serde(default)]
        running: bool,
        #[serde(default)]
        volume: Option<f64>,
    },
    Launch {
        #[serde(default)]
        running: bool,
    },
    Game {
        #[serde(default)]
        running: bool,
    },
    Library(LibraryState),
    Mute {
        #[serde(default)]
        muted: bool,
        #[serde(default)]
        volume: f64,
    },
    AppMute {
        #[serde(default)]
        running: bool,
        #[serde(default)]
        muted: bool,
    },
    Mixer {
        #[serde(default)]
        muted: bool,
        #[serde(default)]
        volume: f64,
    },
    Hold {
        #[serde(default)]
        held: bool,
    },
    Highlight {
        #[serde(default)]
        on: bool,
        #[serde(default)]
        color: Option<String>,
    },
    Error {
        #[serde(default)]
        message: Option<String>,
    },
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OutputOption {
    pub alias: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub connected: bool,
    /// `speaker` or `headphones` when windows-link is told which the device is.
    #[serde(default)]
    pub icon: Option<String>,
}

/// A library button's state (`steam.library`, `dlsite.library`, `fanza.library`).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct LibraryState {
    /// The games pinned to the tab, in the order they were pinned.
    pub pins: Vec<Pin>,
    /// The shop to sign in to before the list can be read.
    pub sign_in: Option<String>,
    /// `whole` when the pictures are shown whole (`DLsite`'s art and program icons).
    pub pictures: Option<String>,
    pub license_keys: bool,
    /// Whether the library runs rounds of downloads and updates to follow.
    pub rounds: bool,
    /// Whether its games may have a shop page.
    pub pages: bool,
}

impl LibraryState {
    pub fn whole_pictures(&self) -> bool {
        self.pictures.as_deref() == Some("whole")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Pin {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Desktop {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub index: i64,
    #[serde(default)]
    pub current: bool,
}

/// A monitor from `GET /touch-monitors`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Monitor {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// The display name, such as `\\.\DISPLAY2`.
    #[serde(default)]
    pub gdi_name: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub touch: bool,
}

/// `GET /buttons/{id}/library`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct Library {
    pub items: Vec<Item>,
    pub labels: Vec<Label>,
    /// Labels whose games show only while the label is selected.
    pub hide: Vec<String>,
    /// Why only the installed games are listed.
    pub partial: Option<String>,
    /// Why labels cannot be changed now.
    pub labels_locked: Option<String>,
    /// The shop to sign in to.
    pub sign_in: Option<String>,
}

/// A game in a library.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct Item {
    pub id: String,
    pub name: String,
    /// The maker (`DLsite`'s circle).
    pub detail: Option<String>,
    pub labels: Vec<String>,
    pub installed: Option<bool>,
    /// A picture elsewhere than windows-link (`DLsite`'s art).
    pub image: Option<String>,
    /// How a download or update windows-link runs goes.
    pub status: Option<String>,
    /// Whether it has several programs to choose from.
    pub choosable: bool,
    /// Its page in the shop.
    pub page: Option<String>,
    /// `DLsite`'s update date (Unix seconds).
    pub updated: Option<i64>,
    /// The purchase date (Unix seconds).
    pub bought: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Label {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub editable: bool,
}

/// `GET /audio/mixer`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Mixer {
    pub master: Level,
    #[serde(default)]
    pub apps: Vec<AppLevel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Level {
    pub volume: f64,
    #[serde(default)]
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AppLevel {
    pub process: String,
    pub name: String,
    pub volume: f64,
    #[serde(default)]
    pub muted: bool,
}

/// `GET …/library/{item}/programs`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Programs {
    pub candidates: Vec<String>,
    pub chosen: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LicenseKey {
    pub label: String,
    pub value: String,
}

/// `GET …/library/update`: a shop's round of downloads and updates.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Round {
    /// `purchases` or `downloads` while a round runs.
    pub doing: Option<String>,
    pub started: Option<i64>,
    pub finished: Option<i64>,
    /// Why the purchases could not be read.
    pub purchases: Option<String>,
    /// Why the games' folder could not be read.
    pub folder: Option<String>,
    pub jobs: Vec<Job>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub status: String,
}

/// windows-link's error body.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct ErrorBody {
    pub error: Option<String>,
    pub message: Option<String>,
}

impl ErrorBody {
    /// An error the panel itself names (the login window, a monitor it does not know).
    pub fn named(error: &str) -> Self {
        Self {
            error: Some(error.into()),
            message: None,
        }
    }

    pub fn is(&self, error: &str) -> bool {
        self.error.as_deref() == Some(error)
    }

    pub fn message(&self) -> &str {
        self.message.as_deref().unwrap_or_default()
    }
}

/// A message from the `/events` socket.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Snapshot {
        buttons: Vec<Button>,
        desktops: Vec<Desktop>,
        unmatched: Vec<String>,
        desktops_error: Option<String>,
    },
    Button(Button),
    Desktops {
        desktops: Vec<Desktop>,
        unmatched: Vec<String>,
        error: Option<String>,
        /// `reconnected` when windows-link reconnected to Explorer, which forgets pins.
        reason: Option<String>,
    },
    Power {
        resting: bool,
    },
}

impl Event {
    /// The message in `text`, or none for one this version does not know.
    pub fn parse(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        let field = |name: &str| value.get(name).cloned().unwrap_or(Value::Null);
        let list =
            |name: &str| -> Vec<String> { serde_json::from_value(field(name)).unwrap_or_default() };
        let text = |name: &str| field(name).as_str().map(str::to_owned);
        match value.get("type")?.as_str()? {
            "snapshot" => Some(Self::Snapshot {
                buttons: serde_json::from_value(field("buttons")).ok()?,
                desktops: serde_json::from_value(field("desktops")).unwrap_or_default(),
                unmatched: list("unmatched"),
                desktops_error: text("desktops_error"),
            }),
            "button" => Some(Self::Button(serde_json::from_value(field("button")).ok()?)),
            "desktops" => Some(Self::Desktops {
                desktops: serde_json::from_value(field("desktops")).ok()?,
                unmatched: list("unmatched"),
                error: text("error"),
                reason: text("reason"),
            }),
            "power" => Some(Self::Power {
                resting: field("resting").as_bool()?,
            }),
            _ => None,
        }
    }
}

/// A state that does not parse is unknown, so one odd button leaves the others readable.
fn lenient<'de, D: Deserializer<'de>>(deserializer: D) -> Result<State, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(State::deserialize(value).unwrap_or_default())
}

fn truthy<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Null => false,
        Value::Bool(b) => b,
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    })
}

#[cfg(test)]
mod tests {
    use super::{Button, Event, State};

    #[test]
    fn reads_a_snapshot_and_keeps_a_button_whose_state_it_cannot_read() {
        let text = r#"{"type":"snapshot","buttons":[
            {"id":"output","type":"audio.output_toggle","label":"出力切替","desktop":null,
             "state":{"kind":"output","current":"motu","current_name":"MOTU","options":[
               {"alias":"motu","name":"MOTU","connected":true,"icon":"speaker"}]}},
            {"id":"odd","label":"Odd","icon":true,"state":{"kind":"volume","running":"yes"}},
            {"id":"future","label":"Future","state":{"kind":"teleport"}}],
          "desktops":[{"id":"G0","name":"dev","index":0,"current":true}],
          "unmatched":["old"],"desktops_error":null,"later":1}"#;
        let Some(Event::Snapshot {
            buttons,
            desktops,
            unmatched,
            desktops_error,
        }) = Event::parse(text)
        else {
            panic!("not a snapshot");
        };
        assert_eq!(buttons.len(), 3, "every button is kept");
        assert!(matches!(buttons[0].state, State::Output { .. }), "output");
        assert_eq!(
            buttons[1].state,
            State::Unknown,
            "a broken state is unknown"
        );
        assert!(buttons[1].icon, "icon");
        assert_eq!(buttons[2].state, State::Unknown, "a new kind is unknown");
        assert_eq!(desktops[0].name, "dev");
        assert_eq!(unmatched, ["old"]);
        assert_eq!(desktops_error, None);
    }

    #[test]
    fn reads_the_other_events_and_ignores_unknown_ones() {
        let button = Event::parse(
            r#"{"type":"button","button":{"id":"m","label":"M","state":{"kind":"mute","muted":true,"volume":0.5}}}"#,
        );
        assert!(
            matches!(
                button,
                Some(Event::Button(Button {
                    state: State::Mute { muted: true, .. },
                    ..
                }))
            ),
            "{button:?}"
        );
        let desktops = Event::parse(
            r#"{"type":"desktops","reason":"reconnected","desktops":[],"error":"no explorer"}"#,
        );
        assert_eq!(
            desktops,
            Some(Event::Desktops {
                desktops: vec![],
                unmatched: vec![],
                error: Some("no explorer".into()),
                reason: Some("reconnected".into()),
            })
        );
        assert_eq!(
            Event::parse(r#"{"type":"power","resting":true}"#),
            Some(Event::Power { resting: true })
        );
        assert_eq!(Event::parse(r#"{"type":"weather"}"#), None);
        assert_eq!(Event::parse("not json"), None);
    }
}
