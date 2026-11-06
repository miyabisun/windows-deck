//! Which buttons a desktop tab shows.

use super::protocol::{Button, Desktop};

/// A desktop's tab shows the shared buttons (no `desktop`) except those that name it in
/// `except`, then the buttons of its own desktop file (`desktop` is the desktop's name).
/// Without desktops (the feature is off in windows-link) every button is shown.
pub fn buttons_for<'a>(buttons: &'a [Button], desktops: &[Desktop]) -> Vec<&'a Button> {
    let Some(current) = desktops.iter().find(|d| d.current) else {
        return buttons.iter().collect();
    };
    let same = |name: &str| name.to_lowercase() == current.name.to_lowercase();
    buttons
        .iter()
        .filter(|b| match &b.desktop {
            Some(desktop) => same(desktop),
            None => !b.except.iter().any(|name| same(name)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Button, Desktop, buttons_for};
    use crate::model::protocol::State;

    fn button(id: &str, desktop: Option<&str>, except: &[&str]) -> Button {
        Button {
            id: id.into(),
            label: id.into(),
            desktop: desktop.map(Into::into),
            except: except.iter().map(|e| (*e).into()).collect(),
            icon: false,
            state: State::Unknown,
        }
    }

    fn buttons() -> Vec<Button> {
        vec![
            button("output", None, &["dev"]),
            button("everywhere", None, &[]),
            button("sf6", Some("SF6"), &[]),
            button("blue-archive", Some("ブルアカ"), &[]),
        ]
    }

    fn desktops(current: &str) -> Vec<Desktop> {
        ["dev", "sf6", "ブルアカ"]
            .iter()
            .enumerate()
            .map(|(index, name)| Desktop {
                id: format!("GUID-{index}"),
                name: (*name).into(),
                index: i64::try_from(index).unwrap(),
                current: *name == current,
            })
            .collect()
    }

    fn ids(list: &[&Button]) -> Vec<String> {
        list.iter().map(|b| b.id.clone()).collect()
    }

    #[test]
    fn shows_a_desktops_own_buttons_after_the_shared_ones() {
        let all = buttons();
        assert_eq!(
            ids(&buttons_for(&all, &desktops("sf6"))),
            ["output", "everywhere", "sf6"]
        );
        assert_eq!(
            ids(&buttons_for(&all, &desktops("ブルアカ"))),
            ["output", "everywhere", "blue-archive"]
        );
    }

    #[test]
    fn leaves_shared_buttons_out_of_the_desktops_they_except() {
        let all = buttons();
        assert_eq!(ids(&buttons_for(&all, &desktops("dev"))), ["everywhere"]);
        let upper = [button("x", None, &["DEV"])];
        assert!(
            buttons_for(&upper, &desktops("dev")).is_empty(),
            "except ignores case"
        );
    }

    #[test]
    fn shows_every_button_without_desktops() {
        let all = buttons();
        assert_eq!(
            ids(&buttons_for(&all, &[])),
            ["output", "everywhere", "sf6", "blue-archive"]
        );
    }
}
