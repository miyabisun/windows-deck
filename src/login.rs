//! Signing in to a shop (FANZA, DLsite): a private window where the user logs in by hand
//! (the shops' bot checks pass in a real browser). Its cookies go to windows-link, which
//! keeps the session from then on; the window keeps none.

use std::sync::{Mutex, mpsc};

use serde::Serialize;
use tauri::{
    AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder, WindowEvent,
    webview::{Cookie, PageLoadEvent},
};
use tracing::info;

/// What differs between the shops' login windows.
pub struct Shop {
    pub label: &'static str,
    pub title: &'static str,
    /// Where the window starts: the shop takes a visitor who is not signed in through its
    /// login page, then back to a page only a signed-in user sees.
    pub start: &'static str,
    /// The site whose cookies (on it and below it) windows-link needs.
    pub site: &'static str,
    /// Whether the window shows that page.
    pub signed_in: fn(&Url) -> bool,
}

/// The shop called `name` in windows-link's `sign_in`, when it has a login window.
pub fn shop(name: &str) -> Option<Shop> {
    match name {
        // FANZA's library, past DMM's age check and login page.
        "fanza" => Some(Shop {
            label: "fanza-login",
            title: "FANZA にログイン",
            start: "https://dlsoft.dmm.co.jp/library/",
            site: "dmm.co.jp",
            signed_in: |url| {
                url.scheme() == "https"
                    && url.host_str() == Some("dlsoft.dmm.co.jp")
                    && url.path().starts_with("/library")
            },
        }),
        // DLsite Play, past DLsite's login (its own `/login/` sends a visitor there).
        "dlsite" => Some(Shop {
            label: "dlsite-login",
            title: "DLsite にログイン",
            start: "https://play.dlsite.com/login/",
            site: "dlsite.com",
            signed_in: |url| {
                url.scheme() == "https"
                    && url.host_str() == Some("play.dlsite.com")
                    && !url.path().starts_with("/login")
            },
        }),
        _ => None,
    }
}

/// A cookie for windows-link, as `PUT /{shop}/session` takes it.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct SessionCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    /// Unix time; `None` for a cookie that lasts the session.
    pub expires: Option<i64>,
    pub secure: bool,
    pub http_only: bool,
}

/// A cookie of `site` (on it or below it), the ones windows-link needs.
pub fn session_cookie(cookie: &Cookie<'_>, site: &str) -> Option<SessionCookie> {
    let domain = cookie.domain()?;
    let bare = domain.trim_start_matches('.');
    let below = bare
        .strip_suffix(site)
        .is_some_and(|rest| rest.ends_with('.'));
    if bare != site && !below {
        return None;
    }
    Some(SessionCookie {
        name: cookie.name().to_owned(),
        value: cookie.value().to_owned(),
        domain: domain.to_owned(),
        path: cookie.path().unwrap_or("/").to_owned(),
        expires: cookie
            .expires_datetime()
            .map(tauri::webview::cookie::time::OffsetDateTime::unix_timestamp),
        secure: cookie.secure().unwrap_or(false),
        http_only: cookie.http_only().unwrap_or(false),
    })
}

/// Show a window on every virtual desktop through windows-link at `link` (a new window
/// of the panel may otherwise stay hidden on another desktop), and bring it forward.
async fn pin(window: &tauri::WebviewWindow, link: &str) {
    // The handle itself cannot wait across threads; its number can.
    let handle = window.hwnd().map(|hwnd| hwnd.0 as isize);
    match handle {
        Ok(handle) => {
            let url = format!("{link}/windows/{handle}/pin");
            let pinned =
                tauri::async_runtime::spawn_blocking(move || ureq::post(&url).send_empty()).await;
            if !matches!(pinned, Ok(Ok(_))) {
                tracing::warn!("cannot show the login window on every desktop");
            }
        }
        Err(err) => tracing::warn!(%err, "the login window has no handle"),
    }
    let _ = window.set_focus();
}

/// Open `shop`'s login window and wait until the user has signed in, then return the
/// shop's cookies and close it. An error when the window is closed first.
#[tauri::command]
pub async fn shop_login(
    app: AppHandle,
    link: String,
    shop: String,
) -> Result<Vec<SessionCookie>, String> {
    let fail = |err: tauri::Error| err.to_string();
    let Some(shop) = self::shop(&shop) else {
        return Err(format!("{shop} has no login window"));
    };
    if let Some(open) = app.get_webview_window(shop.label) {
        open.destroy().map_err(fail)?;
    }
    let (done, signed) = mpsc::channel::<bool>();
    let on_load = Mutex::new(Some(done.clone()));
    let url = Url::parse(shop.start).map_err(|err| err.to_string())?;
    let signed_in = shop.signed_in;
    let window = WebviewWindowBuilder::new(&app, shop.label, WebviewUrl::External(url))
        .title(shop.title)
        .inner_size(1000.0, 820.0)
        .center()
        .incognito(true)
        .on_page_load(move |_, payload| {
            if payload.event() == PageLoadEvent::Finished && signed_in(payload.url()) {
                let sender = on_load
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take();
                if let Some(sender) = sender {
                    let _ = sender.send(true);
                }
            }
        })
        .build()
        .map_err(fail)?;
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            let _ = done.send(false);
        }
    });
    pin(&window, &link).await;
    let wait = tauri::async_runtime::spawn_blocking(move || signed.recv().unwrap_or(false));
    if !wait.await.map_err(fail)? {
        return Err("the login window was closed before signing in".into());
    }
    // WebView2 deadlocks reading cookies on the thread that handles the window.
    let reader = window.clone();
    let cookies = tauri::async_runtime::spawn_blocking(move || reader.cookies())
        .await
        .map_err(fail)?
        .map_err(fail)?;
    let _ = window.destroy();
    let cookies: Vec<SessionCookie> = cookies
        .iter()
        .filter_map(|cookie| session_cookie(cookie, shop.site))
        .collect();
    info!(
        site = shop.site,
        cookies = cookies.len(),
        "signed in to a shop"
    );
    Ok(cookies)
}

#[cfg(test)]
mod tests {
    use tauri::webview::Cookie;

    use super::{SessionCookie, session_cookie, shop};

    #[test]
    fn fanzas_library_page_means_signed_in() {
        let fanza = shop("fanza").unwrap();
        let yes = |url: &str| (fanza.signed_in)(&url.parse().unwrap());
        assert!(yes("https://dlsoft.dmm.co.jp/library/"));
        assert!(yes(
            "https://dlsoft.dmm.co.jp/library/detail/single/selen_0004/"
        ));
        // DMM's login and age check pages on the way.
        assert!(!yes(
            "https://accounts.dmm.co.jp/service/login/password?path=x"
        ));
        assert!(!yes("https://www.dmm.co.jp/age_check/=/?rurl=x"));
        assert!(!yes("http://dlsoft.dmm.co.jp/library/"));
    }

    #[test]
    fn dlsite_play_past_the_login_means_signed_in() {
        let dlsite = shop("dlsite").unwrap();
        let yes = |url: &str| (dlsite.signed_in)(&url.parse().unwrap());
        assert!(yes("https://play.dlsite.com/"));
        assert!(yes("https://play.dlsite.com/library"));
        // Its own way in, DLsite's login with its Cloudflare check, and the shop.
        assert!(!yes("https://play.dlsite.com/login/"));
        assert!(!yes("https://login.dlsite.com/login"));
        assert!(!yes("https://www.dlsite.com/home/login/=/_query/x"));
        assert!(!yes("http://play.dlsite.com/"));
        assert!(shop("steam").is_none());
    }

    #[test]
    fn keeps_the_shops_cookies_with_their_lifetime() {
        let login = Cookie::parse(
            "login_secure_id=abc; Domain=.dmm.co.jp; Path=/; Expires=Wed, 06 Oct 2027 17:00:00 GMT; Secure; HttpOnly",
        )
        .unwrap();
        assert_eq!(
            session_cookie(&login, "dmm.co.jp"),
            Some(SessionCookie {
                name: "login_secure_id".into(),
                value: "abc".into(),
                domain: "dmm.co.jp".into(),
                path: "/".into(),
                expires: Some(1_822_842_000),
                secure: true,
                http_only: true,
            })
        );
        let session = Cookie::parse("laravel_session=x; Domain=dlsoft.dmm.co.jp").unwrap();
        let kept = session_cookie(&session, "dmm.co.jp").unwrap();
        assert_eq!((kept.path.as_str(), kept.expires), ("/", None));
        let play = Cookie::parse("play_session=x; Domain=play.dlsite.com").unwrap();
        assert!(session_cookie(&play, "dlsite.com").is_some());
        // Other sites' cookies, and look-alike domains, stay out.
        for (other, site) in [
            ("a=1; Domain=.google.com", "dmm.co.jp"),
            ("a=1; Domain=evildmm.co.jp", "dmm.co.jp"),
            ("a=1", "dmm.co.jp"),
            ("a=1; Domain=.dmm.co.jp", "dlsite.com"),
            ("a=1; Domain=notdlsite.com", "dlsite.com"),
        ] {
            assert_eq!(
                session_cookie(&Cookie::parse(other).unwrap(), site),
                None,
                "{other}"
            );
        }
    }
}
