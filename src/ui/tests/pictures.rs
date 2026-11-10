//! Games' pictures in their tiles, read in at their real shapes: `DLsite`'s 4:3 art, FANZA's
//! tall and wide pictures and Steam's 460:215 headers, pinned to a tab and in a library's list.

use std::sync::{Arc, Mutex};

use futures::{FutureExt as _, future::BoxFuture};
use gpui::{Bounds, Pixels, TestAppContext, VisualTestContext, px};
use http_client::{AsyncBody, HttpClient, Request, Response, Url, http::HeaderValue};
use serde_json::{Value, json};

use super::{bounds, click, has, open, slide};
use crate::{
    link::mock::{BASE, MockLink, dlsite_button, library_button},
    model::library::picture_url,
    ui::library::PICTURES,
};

/// Pictures of the size each URL names (the first of `sizes` whose text is in the URL), drawn as
/// SVG, which GPUI reads like any other picture.
struct Shapes(Vec<(&'static str, u32, u32)>, Requests);

/// The URLs read so far, in order.
type Requests = Arc<Mutex<Vec<String>>>;

impl HttpClient for Shapes {
    fn user_agent(&self) -> Option<&HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(
        &self,
        request: Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let url = request.uri().to_string();
        self.1.lock().unwrap().push(url.clone());
        let (width, height) = self
            .0
            .iter()
            .find(|(part, ..)| url.contains(part))
            .map_or((460, 215), |&(_, w, h)| (w, h));
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"><rect width="{width}" height="{height}" fill="#c44"/></svg>"##
        );
        async move { Ok(Response::builder().status(200).body(AsyncBody::from(svg))?) }.boxed()
    }
}

fn with_pictures(cx: &mut TestAppContext, sizes: Vec<(&'static str, u32, u32)>) -> Requests {
    let requests = Requests::default();
    cx.update(|cx| cx.set_http_client(Arc::new(Shapes(sizes, requests.clone()))));
    requests
}

fn read(vcx: &mut VisualTestContext, selector: &str) -> Bounds<Pixels> {
    bounds(vcx, selector).unwrap_or_else(|| panic!("{selector} is not on the screen"))
}

/// Where a tile's picture sits against its frame and its name.
fn placed(
    vcx: &mut VisualTestContext,
    key: &str,
    name: &str,
) -> (Bounds<Pixels>, Bounds<Pixels>, Pixels) {
    let picture = read(vcx, &format!("picture-{key}"));
    let frame = read(vcx, &format!("frame-{key}"));
    let name = read(vcx, &format!("text:{name}"));
    (picture, frame, name.top())
}

const NEAR: f32 = 0.5;

fn near(a: Pixels, b: Pixels) -> bool {
    (f32::from(a) - f32::from(b)).abs() <= NEAR
}

/// Every game's picture the same way, whatever the shop and the picture's shape: a 460:215
/// frame against the tile's top and sides (inside its 1px border), the picture filling the
/// frame (and drawn whole inside it), above the name.
fn assert_framed(vcx: &mut VisualTestContext, key: &str, name: &str) {
    let (picture, frame, name_top) = placed(vcx, key, name);
    let tile = read(vcx, &format!("tile-{key}"));
    assert!(
        near(frame.size.height, frame.size.width * (215. / 460.)),
        "{key}: the frame is 460:215, {frame:?}"
    );
    let border = px(1.);
    assert!(
        near(frame.left(), tile.left() + border)
            && near(frame.top(), tile.top() + border)
            && near(frame.right(), tile.right() - border),
        "{key}: the frame {frame:?} meets the top and sides of its tile {tile:?}"
    );
    assert!(
        near(picture.left(), frame.left())
            && near(picture.top(), frame.top())
            && near(picture.right(), frame.right())
            && near(picture.bottom(), frame.bottom()),
        "{key}: the picture {picture:?} fills its frame {frame:?}"
    );
    assert!(
        picture.bottom() <= name_top + px(NEAR),
        "{key}: the picture ends {:?}, above the name at {name_top:?}",
        picture.bottom()
    );
}

#[gpui::test]
fn dlsite_and_fanza_pictures_sit_as_steam_headers_do(cx: &mut TestAppContext) {
    // a1 is DLsite's 4:3 art; b2 and c3 are FANZA's tall and wide pictures.
    with_pictures(
        cx,
        vec![("a1", 560, 420), ("b2", 411, 560), ("c3", 822, 560)],
    );
    let mock = MockLink::default();
    mock.state().buttons.push(dlsite_button(json!([
        { "id": "a1", "name": "湿度の高い夏のマゾ" },
        { "id": "b2", "name": "催眠アプリ" }
    ])));
    let (_deck, vcx) = open(cx, &mock);
    assert_framed(vcx, "dlsite/a1", "湿度の高い夏のマゾ");
    assert_framed(vcx, "dlsite/b2", "催眠アプリ");
    click(vcx, "tile-dlsite");
    assert_framed(vcx, "list-dlsite/a1", "湿度の高い夏のマゾ");
    assert_framed(vcx, "list-dlsite/b2", "催眠アプリ");
    assert_framed(vcx, "list-dlsite/c3", "壊れたゲーム");
}

#[gpui::test]
fn a_steam_header_fills_its_frame(cx: &mut TestAppContext) {
    with_pictures(cx, vec![]);
    let mock = MockLink::default();
    mock.state().buttons.push(library_button(
        json!([{ "id": "646570", "name": "Slay the Spire" }]),
    ));
    let (_deck, vcx) = open(cx, &mock);
    assert_framed(vcx, "games/646570", "Slay the Spire");
    click(vcx, "tile-games");
    assert_framed(vcx, "list-games/646570", "Slay the Spire");
}

/// Slide the list (`dy` a time) until `game` is on the screen.
fn slide_to(vcx: &mut VisualTestContext, game: &str, dy: f32) {
    for _ in 0..60 {
        if has(vcx, game) {
            return;
        }
        slide(vcx, "games-area", 0., dy);
    }
    panic!("{game} never came into view");
}

/// A Steam library of `games` games, 1000 and up, all in the list as it opens.
fn long_library(games: usize) -> MockLink {
    let mock = MockLink::default();
    mock.state().buttons.push(library_button(json!([])));
    mock.state().library["items"] = Value::Array(
        (0..games)
            .map(|i| json!({ "id": (1000 + i).to_string(), "name": format!("Game {i}"), "installed": true, "labels": [] }))
            .collect(),
    );
    mock
}

#[gpui::test]
fn a_long_list_keeps_only_the_pictures_used_lately(cx: &mut TestAppContext) {
    let requests = with_pictures(cx, vec![]);
    let read = |id: u32| {
        let url = picture_url(BASE, "games", &id.to_string());
        requests
            .lock()
            .unwrap()
            .iter()
            .filter(|u| **u == url)
            .count()
    };
    let games = 200;
    let mock = long_library(games);
    let (deck, vcx) = open(cx, &mock);
    click(vcx, "tile-games");
    let kept = |vcx: &mut VisualTestContext| {
        vcx.run_until_parked();
        deck.read_with(vcx, |deck, cx| {
            deck.library()
                .expect("the list is open")
                .read(cx)
                .pictures_kept(cx)
        })
    };
    let (first, last) = ("tile-list-games/1000", "tile-list-games/1199");
    assert!(has(vcx, first), "the first rows are shown");
    assert_eq!(read(1000), 1, "the first game's picture is read");
    let at_first = requests.lock().unwrap().len();
    assert!(at_first < games, "only the rows near the screen are read");

    // Drawing the same screen again reads nothing again.
    for _ in 0..5 {
        vcx.update(|window, _| window.refresh());
        vcx.run_until_parked();
    }
    assert_eq!(
        requests.lock().unwrap().len(),
        at_first,
        "nothing is read twice"
    );

    slide_to(vcx, last, -600.);
    assert_eq!(read(1199), 1, "the last game's picture is read");
    assert!(
        kept(vcx) <= PICTURES,
        "{} pictures are kept at the end, at most {PICTURES}",
        kept(vcx)
    );
    assert!(
        requests.lock().unwrap().len() > PICTURES,
        "more pictures than are kept have been read on the way"
    );
    let at_last = requests.lock().unwrap().len();
    for _ in 0..5 {
        vcx.update(|window, _| window.refresh());
        vcx.run_until_parked();
    }
    assert_eq!(
        requests.lock().unwrap().len(),
        at_last,
        "the pictures on the screen stay: nothing is read while the list rests"
    );

    // Back at the top, the first game's picture was let go and is read again, once.
    slide_to(vcx, first, 600.);
    assert_eq!(read(1000), 2, "the first game's picture is read again");
    assert_framed(vcx, "list-games/1000", "Game 0");
    assert!(kept(vcx) <= PICTURES, "still at most {PICTURES}");
}

#[gpui::test]
fn a_screen_of_more_pictures_than_are_kept_reads_each_once(cx: &mut TestAppContext) {
    let requests = with_pictures(cx, vec![]);
    let games = 200;
    let (deck, vcx) = open(cx, &long_library(games));
    // A screen large enough to draw every game at once: more than the list keeps.
    vcx.simulate_resize(gpui::size(px(3840.), px(4800.)));
    click(vcx, "tile-games");
    for _ in 0..5 {
        vcx.update(|window, _| window.refresh());
        vcx.run_until_parked();
    }
    assert!(
        has(vcx, "tile-list-games/1000") && has(vcx, "tile-list-games/1199"),
        "the first and the last game are on the screen"
    );
    assert!(games > PICTURES, "more games than the list keeps");
    assert_eq!(
        requests.lock().unwrap().len(),
        games,
        "every picture on the screen is read once and stays"
    );
    let kept = deck.read_with(vcx, |deck, cx| {
        deck.library()
            .expect("the list is open")
            .read(cx)
            .pictures_kept(cx)
    });
    assert_eq!(kept, games, "all of them are held while they are drawn");
}
