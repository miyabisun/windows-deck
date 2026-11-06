//! Games' pictures in their tiles, read in at their real shapes: `DLsite`'s 4:3 art, FANZA's
//! tall and wide pictures and Steam's 460:215 headers, pinned to a tab and in a library's list.

use std::sync::Arc;

use futures::{FutureExt as _, future::BoxFuture};
use gpui::{Bounds, Pixels, TestAppContext, VisualTestContext, px};
use http_client::{AsyncBody, HttpClient, Request, Response, Url, http::HeaderValue};
use serde_json::json;

use super::{bounds, click, open};
use crate::{
    link::mock::{MockLink, dlsite_button, library_button},
    ui::theme::SP_3,
};

/// Pictures of the size each URL names (the first of `sizes` whose text is in the URL), drawn as
/// SVG, which GPUI reads like any other picture.
struct Shapes(Vec<(&'static str, u32, u32)>);

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

fn with_pictures(cx: &mut TestAppContext, sizes: Vec<(&'static str, u32, u32)>) {
    cx.update(|cx| cx.set_http_client(Arc::new(Shapes(sizes))));
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

/// A picture shown whole: inside its 460:215 frame, 12px in from each side, above the name.
fn assert_whole(vcx: &mut VisualTestContext, key: &str, name: &str) {
    let (picture, frame, name_top) = placed(vcx, key, name);
    assert!(
        near(frame.size.height, frame.size.width * (215. / 460.)),
        "{key}: the frame is 460:215, {frame:?}"
    );
    let inside = Bounds {
        origin: frame.origin + gpui::point(SP_3, SP_3),
        size: frame.size - gpui::size(SP_3 * 2., SP_3 * 2.),
    };
    assert!(
        near(picture.left(), inside.left())
            && near(picture.top(), inside.top())
            && near(picture.right(), inside.right())
            && near(picture.bottom(), inside.bottom()),
        "{key}: the picture {picture:?} is 12px inside its frame {frame:?}"
    );
    assert!(
        picture.bottom() <= name_top,
        "{key}: the picture ends {:?}, above the name at {name_top:?}",
        picture.bottom()
    );
}

#[gpui::test]
fn dlsite_and_fanza_pictures_stay_inside_their_frames(cx: &mut TestAppContext) {
    // a1 is DLsite's 4:3 art; b2 and c3 are FANZA's tall and wide pictures.
    with_pictures(
        cx,
        vec![("a1", 400, 300), ("b2", 411, 560), ("c3", 822, 560)],
    );
    let mock = MockLink::default();
    mock.state().buttons.push(dlsite_button(json!([
        { "id": "a1", "name": "湿度の高い夏のマゾ" },
        { "id": "b2", "name": "催眠アプリ" }
    ])));
    let (_deck, vcx) = open(cx, &mock);
    assert_whole(vcx, "dlsite/a1", "湿度の高い夏のマゾ");
    assert_whole(vcx, "dlsite/b2", "催眠アプリ");
    click(vcx, "tile-dlsite");
    assert_whole(vcx, "list-dlsite/a1", "湿度の高い夏のマゾ");
    assert_whole(vcx, "list-dlsite/b2", "催眠アプリ");
    assert_whole(vcx, "list-dlsite/c3", "壊れたゲーム");
}

#[gpui::test]
fn a_steam_header_fills_its_frame(cx: &mut TestAppContext) {
    with_pictures(cx, vec![]);
    let mock = MockLink::default();
    mock.state().buttons.push(library_button(
        json!([{ "id": "646570", "name": "Slay the Spire" }]),
    ));
    let (_deck, vcx) = open(cx, &mock);
    let (picture, frame, name_top) = placed(vcx, "games/646570", "Slay the Spire");
    assert!(
        near(picture.left(), frame.left())
            && near(picture.top(), frame.top())
            && near(picture.size.width, frame.size.width)
            && near(picture.size.height, frame.size.height),
        "the header {picture:?} fills its frame {frame:?}"
    );
    assert!(picture.bottom() <= name_top + px(NEAR), "above the name");
}
