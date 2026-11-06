//! Which monitor the panel goes to, from windows-link's `GET /touch-monitors`.

use super::protocol::Monitor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// Fill the monitor with this display name (`\.\DISPLAY2`); `id` is windows-link's.
    Fill { display: String, id: String },
    /// The configured monitor is not there yet.
    Wait { reason: String },
    /// No monitor is configured: ask for one among these.
    Setup { candidates: Vec<Monitor> },
}

/// The panel fills only the configured monitor. While that one is not there (not connected, or
/// windows-link has not answered) it waits instead of using another, and without a configured
/// monitor it asks for one.
pub fn placement(monitors: Option<&[Monitor]>, preference: Option<&str>) -> Placement {
    let wanted = preference
        .map(|p| p.trim().to_lowercase())
        .unwrap_or_default();
    if wanted.is_empty() {
        return Placement::Setup {
            candidates: monitors.map(<[Monitor]>::to_vec).unwrap_or_default(),
        };
    }
    let Some(monitors) = monitors else {
        return Placement::Wait {
            reason: "windows-link has not reported the monitors".into(),
        };
    };
    let matches = |value: &str| value.trim().to_lowercase() == wanted;
    match monitors
        .iter()
        .find(|m| matches(&m.id) || matches(&m.name) || matches(&m.gdi_name))
    {
        Some(m) => Placement::Fill {
            display: m.gdi_name.clone(),
            id: m.id.clone(),
        },
        None => Placement::Wait {
            reason: format!(
                "monitor {} is not connected",
                preference.unwrap_or_default()
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{Monitor, Placement, placement};

    fn display(n: u8) -> String {
        format!(r"\.\DISPLAY{n}")
    }

    fn monitors() -> Vec<Monitor> {
        let monitor = |id: &str, name: &str, n, primary, touch| Monitor {
            id: id.into(),
            name: name.into(),
            gdi_name: display(n),
            primary,
            touch,
        };
        vec![
            monitor("mon-1", "U13NA", 1, false, false),
            monitor("mon-3", "25M2S", 3, true, true),
            monitor("mon-2", "T10FA        ", 2, false, true),
        ]
    }

    fn fill(n: u8, id: &str) -> Placement {
        Placement::Fill {
            display: display(n),
            id: id.into(),
        }
    }

    #[test]
    fn fills_the_configured_monitor_by_id_name_or_display_name() {
        let all = monitors();
        assert_eq!(placement(Some(&all), Some("mon-1")), fill(1, "mon-1"));
        assert_eq!(placement(Some(&all), Some("t10fa")), fill(2, "mon-2"));
        assert_eq!(
            placement(Some(&all), Some(r"\.\display3")),
            fill(3, "mon-3")
        );
    }

    #[test]
    fn waits_rather_than_using_another_monitor() {
        let all = monitors();
        assert_eq!(
            placement(Some(&all), Some("10-inch")),
            Placement::Wait {
                reason: "monitor 10-inch is not connected".into()
            }
        );
        assert_eq!(
            placement(None, Some("mon-2")),
            Placement::Wait {
                reason: "windows-link has not reported the monitors".into()
            }
        );
    }

    #[test]
    fn asks_for_the_setting_without_a_configured_monitor() {
        let all = monitors();
        let one: Vec<Monitor> = all.iter().filter(|m| m.id != "mon-3").cloned().collect();
        assert_eq!(
            placement(Some(&one), None),
            Placement::Setup {
                candidates: one.clone()
            }
        );
        assert_eq!(
            placement(Some(&all), Some("  ")),
            Placement::Setup {
                candidates: all.clone()
            }
        );
        assert_eq!(
            placement(None, None),
            Placement::Setup { candidates: vec![] }
        );
    }
}
