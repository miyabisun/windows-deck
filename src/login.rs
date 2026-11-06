//! Signing in to a shop (FANZA, DLsite): a private browser window where the user logs in by
//! hand (the shops' bot checks pass in a real browser). Its cookies go to windows-link, which
//! keeps the session from then on; the window keeps none.
//!
//! The window runs in a process of its own (`windows-deck --login <shop> --link <url>`), so the
//! panel itself never loads WebView2: the process prints the cookies, or why there are none,
//! as one line of JSON and exits.

use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use serde::Serialize;
use serde_json::{Value, json};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use tracing::info;
use url::Url;
use wry::{PageLoadEvent, WebContext, WebViewBuilder, cookie::Cookie};

use crate::{link::SignIn, model::protocol::ErrorBody};

/// The argument that makes the exe a shop's login window.
pub const LOGIN: &str = "--login";
/// windows-link's URL, for the login window to be shown on every desktop.
pub const LINK: &str = "--link";

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
        expires: cookie.expires_datetime().map(|at| at.unix_timestamp()),
        secure: cookie.secure().unwrap_or(false),
        http_only: cookie.http_only().unwrap_or(false),
    })
}

/// Where WebView2 keeps its files: the folder the Tauri panel used, so none is added.
fn data_directory() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("io.github.miyabisun.windows-deck").join("EBWebView"))
}

enum Signal {
    SignedIn,
}

/// The login window's process: open `shop`'s login, wait until the user has signed in (or
/// closed the window), print the cookies or why not, and return the exit code.
pub fn run(shop: &str, link: &str) -> i32 {
    let Some(shop) = self::shop(shop) else {
        return answer(&json!({ "error": "window", "message": format!("{shop} has no login window") }));
    };
    let event_loop = EventLoopBuilder::<Signal>::with_user_event().build();
    let window = match WindowBuilder::new()
        .with_title(shop.title)
        .with_inner_size(tao::dpi::LogicalSize::new(1000.0, 820.0))
        .build(&event_loop)
    {
        Ok(window) => window,
        Err(err) => return answer(&json!({ "error": "window", "message": err.to_string() })),
    };
    if let Some(monitor) = window.primary_monitor() {
        let size = window.outer_size();
        let area = monitor.size();
        let at = monitor.position();
        let half = |whole: u32, part: u32| (i32::try_from(whole).unwrap_or(0) - i32::try_from(part).unwrap_or(0)) / 2;
        window.set_outer_position(tao::dpi::PhysicalPosition::new(
            at.x + half(area.width, size.width),
            at.y + half(area.height, size.height),
        ));
    }
    let proxy = event_loop.create_proxy();
    let signed_in = shop.signed_in;
    let mut context = WebContext::new(data_directory());
    let webview = match WebViewBuilder::new_with_web_context(&mut context)
        .with_url(shop.start)
        .with_incognito(true)
        .with_on_page_load_handler(move |event, url| {
            if matches!(event, PageLoadEvent::Finished) && Url::parse(&url).is_ok_and(|url| signed_in(&url)) {
                let _ = proxy.send_event(Signal::SignedIn);
            }
        })
        .build(&window)
    {
        Ok(webview) => webview,
        Err(err) => return answer(&json!({ "error": "window", "message": err.to_string() })),
    };
    pin(&window, link);
    let site = shop.site;
    let code = Arc::new(Mutex::new(0));
    let exit = code.clone();
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        let finish = |value: &Value, flow: &mut ControlFlow| {
            let code = answer(value);
            *exit.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = code;
            *flow = ControlFlow::ExitWithCode(code);
        };
        match event {
            Event::UserEvent(Signal::SignedIn) => match webview.cookies() {
                Ok(cookies) => {
                    let cookies: Vec<SessionCookie> =
                        cookies.iter().filter_map(|cookie| session_cookie(cookie, site)).collect();
                    info!(site, cookies = cookies.len(), "signed in to a shop");
                    finish(&json!({ "cookies": cookies }), flow);
                }
                Err(err) => finish(&json!({ "error": "window", "message": err.to_string() }), flow),
            },
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                finish(&json!({ "error": "closed" }), flow);
            }
            _ => {}
        }
    })
}

/// Show the window on every virtual desktop through windows-link at `link` (a new window may
/// otherwise stay on another desktop), and bring it forward.
fn pin(window: &tao::window::Window, link: &str) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window.window_handle().ok().and_then(|h| match h.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    });
    match handle {
        Some(hwnd) => {
            if ureq::post(&format!("{link}/windows/{hwnd}/pin")).send_empty().is_err() {
                tracing::warn!("cannot show the login window on every desktop");
            }
        }
        None => tracing::warn!("the login window has no handle"),
    }
    window.set_focus();
}

fn answer(value: &Value) -> i32 {
    let mut out = std::io::stdout().lock();
    let written = writeln!(out, "{value}").and_then(|()| out.flush());
    i32::from(written.is_err())
}

/// Signs in through the login window's process.
pub struct LoginProcess;

impl SignIn for LoginProcess {
    fn sign_in(&self, shop: &str, base: &str) -> Result<Value, ErrorBody> {
        let window = |message: String| ErrorBody { error: Some("window".into()), message: Some(message) };
        let exe = std::env::current_exe().map_err(|err| window(err.to_string()))?;
        let output = Command::new(exe)
            .args([LOGIN, shop, LINK, base])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .map_err(|err| window(err.to_string()))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let answer: Value = text
            .lines()
            .rev()
            .find_map(|line| serde_json::from_str(line).ok())
            .ok_or_else(|| window(format!("the login window ended without an answer ({})", output.status)))?;
        match answer.get("cookies") {
            Some(cookies) => Ok(cookies.clone()),
            None => Err(serde_json::from_value(answer).unwrap_or_else(|_| ErrorBody::named("window"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use wry::cookie::Cookie;

    use super::{SessionCookie, session_cookie, shop};

    #[test]
    fn fanzas_library_page_means_signed_in() {
        let fanza = shop("fanza").unwrap();
        let yes = |url: &str| (fanza.signed_in)(&url::Url::parse(url).unwrap());
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
        let yes = |url: &str| (dlsite.signed_in)(&url::Url::parse(url).unwrap());
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
