//! Signing in to FANZA: a private window where the user logs in to DMM by hand (DMM's bot
//! checks pass in a real browser). Its cookies go to windows-link, which keeps the
//! session from then on; the window keeps none.

use std::sync::{Mutex, mpsc};

use serde::Serialize;
use tauri::{
    AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder, WindowEvent,
    webview::{Cookie, PageLoadEvent},
};
use tracing::info;

const LABEL: &str = "fanza-login";
/// FANZA's library. DMM takes a visitor who is not signed in through its age check and
/// login page, then back here.
const LIBRARY: &str = "https://dlsoft.dmm.co.jp/library/";

/// A cookie for windows-link, as `PUT /fanza/session` takes it.
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

/// Whether the window shows FANZA's library, which DMM shows only to a signed-in user.
pub fn signed_in(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("dlsoft.dmm.co.jp")
        && url.path().starts_with("/library")
}

/// DMM's cookies (on `dmm.co.jp` and its subdomains), the ones windows-link needs.
pub fn session_cookie(cookie: &Cookie<'_>) -> Option<SessionCookie> {
    let domain = cookie.domain()?;
    let bare = domain.trim_start_matches('.');
    if bare != "dmm.co.jp" && !bare.ends_with(".dmm.co.jp") {
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
                tracing::warn!("cannot show the FANZA login window on every desktop");
            }
        }
        Err(err) => tracing::warn!(%err, "the FANZA login window has no handle"),
    }
    let _ = window.set_focus();
}

/// Open the login window and wait until the user has signed in, then return DMM's
/// cookies and close it. An error when the window is closed first.
#[tauri::command]
pub async fn fanza_login(app: AppHandle, link: String) -> Result<Vec<SessionCookie>, String> {
    let fail = |err: tauri::Error| err.to_string();
    if let Some(open) = app.get_webview_window(LABEL) {
        open.destroy().map_err(fail)?;
    }
    let (done, signed) = mpsc::channel::<bool>();
    let on_load = Mutex::new(Some(done.clone()));
    let url = Url::parse(LIBRARY).map_err(|err| err.to_string())?;
    let window = WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::External(url))
        .title("FANZA にログイン")
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
    let cookies: Vec<SessionCookie> = cookies.iter().filter_map(session_cookie).collect();
    info!(cookies = cookies.len(), "signed in to FANZA");
    Ok(cookies)
}

#[cfg(test)]
mod tests {
    use tauri::webview::Cookie;

    use super::{SessionCookie, session_cookie, signed_in};

    #[test]
    fn the_library_page_means_signed_in() {
        let yes = |url: &str| signed_in(&url.parse().unwrap());
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
    fn keeps_dmms_cookies_with_their_lifetime() {
        let login = Cookie::parse(
            "login_secure_id=abc; Domain=.dmm.co.jp; Path=/; Expires=Wed, 06 Oct 2027 17:00:00 GMT; Secure; HttpOnly",
        )
        .unwrap();
        assert_eq!(
            session_cookie(&login),
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
        let kept = session_cookie(&session).unwrap();
        assert_eq!((kept.path.as_str(), kept.expires), ("/", None));
        // Other sites' cookies, and look-alike domains, stay out.
        for other in [
            "a=1; Domain=.google.com",
            "a=1; Domain=evildmm.co.jp",
            "a=1",
        ] {
            assert_eq!(session_cookie(&Cookie::parse(other).unwrap()), None);
        }
    }
}
