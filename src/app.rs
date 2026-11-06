//! Starting the panel: the configuration, the connection, waiting for the configured monitor,
//! the window filling it, later starts, and automatic updates.

use std::{rc::Rc, sync::Arc, time::Duration};

use futures::{StreamExt as _, channel::mpsc};
use gpui::{
    AnyWindowHandle, App, AppContext as _, AsyncApp, Bounds, DisplayId, Entity, Global,
    PlatformInput, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowKind,
    WindowOptions, px, size,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tracing::{info, warn};

use crate::{
    CHECK_UPDATE, config,
    link::{
        Link,
        http::{Tungstenite, Ureq},
    },
    logging,
    login::LoginProcess,
    model::placement::{Placement, placement},
    pictures::Pictures,
    platform,
    ui::widgets::Act,
    ui::{
        self,
        deck::{Deck, Opened, Setup},
        icons::Assets,
        theme::Theme,
    },
    update::{self, Updater, swap},
};

/// How long to wait before looking for the configured monitor again.
const PLACE_RETRY: Duration = Duration::from_secs(5);

/// What the running panel keeps: its connection, its window once placed, and its updater.
struct Panel {
    link: Entity<Link>,
    window: Option<AnyWindowHandle>,
    updater: Arc<Updater>,
    /// Set while the window is being replaced, so that closing it does not quit.
    replacing: bool,
}

impl Global for Panel {}

pub fn run(version: &str) {
    logging::init();
    info!(%version, "starting");
    let updater = Arc::new(Updater::new(version));
    let (starts, mut later) = mpsc::unbounded::<Vec<String>>();
    platform::listen_for_starts(starts);
    update::run_periodically(updater.clone());
    gpui_platform::application()
        .with_assets(Assets)
        .with_http_client(Arc::new(Pictures::default()))
        .run(move |cx: &mut App| {
            cx.set_global(Theme::for_appearance(cx.window_appearance()));
            cx.set_reduce_motion(platform::reduced_motion());
            ui::bind_keys(cx);
            let loaded = config::load(&config::default_path());
            let link = connect(&loaded.link, cx);
            cx.set_global(Panel {
                link,
                window: None,
                updater,
                replacing: false,
            });
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() && !cx.global::<Panel>().replacing {
                    cx.quit();
                }
            })
            .detach();
            place(loaded, cx);
            // A later start hands its arguments over and exits.
            cx.spawn(async move |cx: &mut AsyncApp| {
                while let Some(args) = later.next().await {
                    let () = cx.update(|cx| started_again(&args, cx));
                }
            })
            .detach();
        });
}

fn connect(base: &str, cx: &mut App) -> Entity<Link> {
    cx.new(|cx| {
        let mut link = Link::new(
            base,
            Arc::new(Ureq::default()),
            Arc::new(Tungstenite),
            Arc::new(LoginProcess),
        );
        link.start(cx);
        link
    })
}

fn started_again(args: &[String], cx: &mut App) {
    let panel = cx.global::<Panel>();
    if args.iter().any(|arg| arg == CHECK_UPDATE) {
        let updater = panel.updater.clone();
        std::thread::spawn(move || updater.check_and_apply());
    } else if let Some(window) = panel.window {
        let _ = window.update(cx, |_, window, _| window.activate_window());
    } else {
        info!("started again before the panel was placed");
    }
}

/// Where the window goes, once windows-link has said.
enum Place {
    /// Fill this monitor (its display name and GPUI's id for it).
    Fill(String, DisplayId),
    Setup(Setup),
    Wait(String),
}

/// Fill the configured monitor, waiting (without a window) until windows-link reports it and it
/// is connected; without one configured, show an ordinary window that asks for one.
fn place(loaded: config::Loaded, cx: &mut App) {
    let link = cx.global::<Panel>().link.clone();
    cx.spawn(async move |cx: &mut AsyncApp| {
        let mut waiting_for = None;
        loop {
            let asked = cx.update(|cx| link.update(cx, |l, cx| l.touch_monitors(cx)));
            let monitors = asked.await;
            let place = match placement(monitors.as_deref(), loaded.monitor.as_deref()) {
                Placement::Fill { display, .. } => match platform::monitor_named(&display) {
                    Some(monitor) => Place::Fill(display, DisplayId::new(monitor)),
                    None => Place::Wait(format!("{display} is not connected")),
                },
                Placement::Setup { candidates } => Place::Setup(Setup {
                    path: loaded.path.clone(),
                    candidates,
                }),
                Placement::Wait { reason } => Place::Wait(reason),
            };
            match place {
                Place::Wait(reason) => {
                    if waiting_for.as_ref() != Some(&reason) {
                        info!(%reason, "waiting to show the panel");
                        waiting_for = Some(reason);
                    }
                    cx.background_executor().timer(PLACE_RETRY).await;
                }
                place => {
                    let () = cx.update(|cx| open(place, &loaded, cx));
                    return;
                }
            }
        }
    })
    .detach();
}

fn hwnd_of(window: &Window) -> Option<isize> {
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

fn open(place: Place, loaded: &config::Loaded, cx: &mut App) {
    let titlebar = Some(TitlebarOptions {
        title: Some("windows-deck".into()),
        appears_transparent: true,
        traffic_light_position: None,
    });
    let (options, setup) = match place {
        Place::Fill(name, id) => {
            let bounds = cx.find_display(id).map_or_else(
                || Bounds::centered(None, size(px(1024.), px(640.)), cx),
                |d| d.bounds(),
            );
            info!(monitor = %name, "filling the monitor");
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Fullscreen(bounds)),
                display_id: Some(id),
                titlebar,
                kind: WindowKind::Normal,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                window_background: WindowBackgroundAppearance::Opaque,
                app_id: Some("windows-deck".into()),
                ..WindowOptions::default()
            };
            (options, None)
        }
        Place::Setup(setup) => {
            warn!("no monitor is configured; showing a window that asks for one");
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1024.), px(640.)),
                    cx,
                ))),
                titlebar,
                is_resizable: false,
                window_background: WindowBackgroundAppearance::Opaque,
                app_id: Some("windows-deck".into()),
                ..WindowOptions::default()
            };
            (options, Some(setup))
        }
        Place::Wait(_) => return,
    };
    let link = cx.global::<Panel>().link.clone();
    let (monitor, error) = (loaded.monitor.clone(), loaded.error.clone());
    let reload: Act = Rc::new(reload);
    let opened = cx.open_window(options, move |window, cx| {
        let hwnd = hwnd_of(window);
        if let Some(hwnd) = hwnd {
            take_touches(hwnd, window, cx);
        }
        cx.set_global(Theme::for_appearance(window.appearance()));
        let opened = Opened {
            monitor,
            config_error: error,
            setup,
            hwnd,
            reload,
        };
        cx.new(|cx| Deck::new(link, opened, window, cx))
    });
    match opened {
        Ok(window) => {
            let panel = cx.global_mut::<Panel>();
            panel.window = Some(window.into());
            panel.replacing = false;
            // The window of a freshly updated panel is up: the previous exe is no longer needed.
            if let Some(exe) = panel.updater.installed_exe() {
                swap::remove_old_later(exe);
            }
        }
        Err(err) => warn!(%err, "cannot open the panel's window"),
    }
}

/// Hand the touch screen's fingers to GPUI's gesture recognizer.
fn take_touches(hwnd: isize, window: &mut Window, cx: &mut App) {
    let (sink, mut touches) = mpsc::unbounded();
    platform::take_touches(hwnd, sink);
    window
        .spawn(cx, async move |cx| {
            while let Some(touch) = touches.next().await {
                let _ =
                    cx.update(|window, cx| window.dispatch_event(PlatformInput::Touch(touch), cx));
            }
        })
        .detach();
}

/// Read the configuration again and place the window anew (the setup window's 再読み込み).
fn reload(window: &mut Window, cx: &mut App) {
    let loaded = config::load(&config::default_path());
    if loaded.link != cx.global::<Panel>().link.read(cx).base() {
        let old = cx.global::<Panel>().link.clone();
        old.update(cx, |l, _| l.stop());
        let link = connect(&loaded.link, cx);
        cx.global_mut::<Panel>().link = link;
    }
    let panel = cx.global_mut::<Panel>();
    panel.window = None;
    panel.replacing = true;
    window.remove_window();
    place(loaded, cx);
}
