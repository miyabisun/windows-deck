import { expect, test } from "@playwright/test";
import { dlsiteButton, startMockLink } from "./mock-link.js";

// The dates in the mock are written as they were in Japan.
test.use({ timezoneId: "Asia/Tokyo" });

/** @type {Awaited<ReturnType<typeof startMockLink>>} */
let mock;

test.beforeEach(async () => {
  mock = await startMockLink();
  mock.link.buttons.push(dlsiteButton());
});

test.afterEach(async () => {
  await mock.close();
});

const open = async (page) => {
  await page.goto(`/?link=${encodeURIComponent(mock.url)}`);
  await page.locator('[data-button="dlsite"]').click();
};
const dialog = (page) => page.getByRole("dialog", { name: "DLsite" });
const game = (page, id) => dialog(page).locator(`[data-button="${id}"]`);
const names = (page) => dialog(page).locator(".tile .label").allTextContents();

/** Hold the mouse down on a locator long enough for a long press. */
async function longPress(page, locator) {
  const box = await locator.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.waitForTimeout(700);
  await page.mouse.up();
}

test("games show their icon whole and their maker, and the maker can be searched", async ({
  page,
}) => {
  await open(page);
  await expect
    .poll(() => names(page))
    .toEqual(["湿度の高い夏のマゾ", "催眠アプリ", "壊れたゲーム"]);
  await expect(game(page, "a1")).toContainText("3Djp_Art");
  // A game known on DLsite shows its art from the address the library gave.
  const art = game(page, "a1").locator("img.cover");
  await expect(art).toHaveAttribute("src", `${mock.url}/dlsite-art/a1.svg`);
  await expect.poll(() => art.evaluate((img) => img.naturalWidth)).toBe(560);
  const picture = game(page, "b2").locator("img.cover");
  await expect(picture).toHaveClass(/whole/);
  expect(await picture.evaluate((img) => getComputedStyle(img).objectFit)).toBe("contain");
  await expect(picture).toHaveAttribute("loading", "lazy");
  await page.screenshot({ path: "test-results/dlsite-list.png" });

  await page.keyboard.type("saimin");
  await expect.poll(() => names(page)).toEqual(["催眠アプリ"]);
});

test("a game bought but not downloaded yet says how its download is going", async ({ page }) => {
  mock.link.dlsite.items.unshift({
    id: "RJ7",
    name: "まだ来ないゲーム",
    detail: "Brand",
    choosable: false,
    image: null,
    installed: false,
    labels: [],
    status: "ダウンロード中 40%",
  });
  await open(page);
  const coming = game(page, "RJ7");
  await expect(coming).toContainText("Brand・ダウンロード中 40%");
  await expect(coming).not.toContainText("未インストール");
  await coming.click();
  await expect(coming).toContainText("まだこの PC にありません（ダウンロード中 40%）");
  await expect(dialog(page)).toBeVisible();
  expect(mock.link.started).toEqual([]);
});

test("Enter in the search field starts the first game found", async ({ page }) => {
  await open(page);
  await page.keyboard.type("3djp");
  await expect.poll(() => names(page)).toEqual(["湿度の高い夏のマゾ"]);
  await page.keyboard.press("Enter");
  await expect(dialog(page).getByRole("status").last()).toHaveText(
    "「湿度の高い夏のマゾ」を起動しました",
  );
  await expect(dialog(page)).toBeVisible();
  expect(mock.link.started).toEqual(["a1"]);
});

test("a game with several programs asks once which one starts it", async ({ page }) => {
  await open(page);
  await game(page, "b2").click();
  const chooser = page.getByRole("dialog", { name: "起動ファイルを選ぶ" });
  await expect(chooser).toContainText("起動ファイルを選ぶ: 催眠アプリ");
  const rows = chooser.getByRole("radio");
  await expect(rows).toHaveCount(2);
  const tab = (await page.getByRole("tab").first().boundingBox()).height;
  expect((await rows.first().boundingBox()).height).toBeGreaterThanOrEqual(tab);
  await page.screenshot({ path: "test-results/dlsite-chooser.png" });
  await chooser.getByRole("radio", { name: "startup.exe" }).click();
  // The choice starts the game; the list stays.
  await expect(chooser).toHaveCount(0);
  await expect(dialog(page).getByRole("status").last()).toHaveText("「催眠アプリ」を起動しました");
  await expect(dialog(page)).toBeVisible();
  expect(mock.link.started).toEqual(["b2:startup.exe"]);

  // Next time it starts at once.
  await game(page, "b2").click();
  await expect.poll(() => mock.link.started).toEqual(["b2:startup.exe", "b2:startup.exe"]);
  await expect(chooser).toHaveCount(0);
});

test("a game's menu changes its program only when it has a choice", async ({ page }) => {
  await open(page);
  await longPress(page, game(page, "a1"));
  await expect(page.getByRole("menuitem", { name: "起動ファイルを選ぶ" })).toHaveCount(0);
  await page.keyboard.press("Escape");

  mock.link.dlsite.programs.b2.chosen = "app.exe";
  await longPress(page, game(page, "b2"));
  await page.getByRole("menuitem", { name: "起動ファイルを選ぶ" }).click();
  const chooser = page.getByRole("dialog", { name: "起動ファイルを選ぶ" });
  await expect(chooser.getByRole("radio", { name: "app.exe" })).toHaveAttribute(
    "aria-checked",
    "true",
  );
  await chooser.getByRole("radio", { name: "startup.exe" }).click();
  await expect(chooser).toHaveCount(0);
  expect(mock.link.dlsite.programs.b2.chosen).toBe("startup.exe");
  // Choosing from the menu does not start the game.
  expect(mock.link.started).toEqual([]);
  await expect(dialog(page)).toBeVisible();
});

test("a game's license key is shown with a button that copies it", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await open(page);
  await longPress(page, game(page, "a1"));
  await page.getByRole("menuitem", { name: "シリアル番号" }).click();
  const keys = page.getByRole("dialog", { name: "シリアル番号" });
  await expect(keys).toContainText("湿度の高い夏のマゾ");
  await expect(keys).toContainText("ライセンスキー");
  await expect(keys.getByText("ABCD-1234-EFGH-5678")).toBeVisible();
  await keys.getByRole("button", { name: "ライセンスキーをコピー" }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe("ABCD-1234-EFGH-5678");
  await expect(keys.getByRole("status")).toHaveText("コピーしました");
  // The copy button is as tall as a row, for a finger.
  const tab = (await page.getByRole("tab").first().boundingBox()).height;
  const copy = await keys.getByRole("button", { name: "ライセンスキーをコピー" }).boundingBox();
  expect(copy.height).toBeGreaterThanOrEqual(tab);
  await page.screenshot({ path: "test-results/dlsite-keys.png" });
  await keys.getByRole("button", { name: "閉じる" }).click();
  await expect(keys).toHaveCount(0);
  await expect(dialog(page)).toBeVisible();

  // A game without a key says so.
  await longPress(page, game(page, "b2"));
  await page.getByRole("menuitem", { name: "シリアル番号" }).click();
  await expect(page.getByRole("dialog", { name: "シリアル番号" })).toContainText(
    "このゲームにはシリアル番号がありません",
  );
});

test("the list goes in order of DLsite's update or purchase date, and stays so", async ({
  page,
}) => {
  await open(page);
  const order = dialog(page).getByRole("button", { name: "並び順" });
  await expect(order).toHaveText("最近遊んだ順");
  await expect
    .poll(() => names(page))
    .toEqual(["湿度の高い夏のマゾ", "催眠アプリ", "壊れたゲーム"]);
  await order.click();
  const menu = page.getByRole("menu", { name: "並び順" });
  await expect(menu.getByRole("menuitemradio")).toHaveText([
    "最近遊んだ順",
    "更新日の新しい順",
    "購入日の新しい順",
  ]);
  await expect(menu.getByRole("menuitemradio", { name: "最近遊んだ順" })).toHaveAttribute(
    "aria-checked",
    "true",
  );
  const tab = (await page.getByRole("tab").first().boundingBox()).height;
  for (const row of await menu.getByRole("menuitemradio").all())
    expect((await row.boundingBox()).height).toBeGreaterThanOrEqual(tab);
  await page.screenshot({ path: "test-results/dlsite-order-menu.png" });
  await menu.getByRole("menuitemradio", { name: "更新日の新しい順" }).click();
  await expect(menu).toHaveCount(0);
  await expect(order).toHaveText("更新日の新しい順");
  await expect
    .poll(() => names(page))
    .toEqual(["催眠アプリ", "湿度の高い夏のマゾ", "壊れたゲーム"]);
  // The date the list is in order of heads each note.
  await expect(game(page, "b2")).toContainText("更新 2026/10/28・Saimin Soft");
  await expect(game(page, "a1")).toContainText("更新 2026/10/16・3Djp_Art");
  await expect(game(page, "c3").locator(".note")).toHaveText("Other");
  await page.screenshot({ path: "test-results/dlsite-order-updated.png" });

  await order.click();
  await page.getByRole("menuitemradio", { name: "購入日の新しい順" }).click();
  await expect
    .poll(() => names(page))
    .toEqual(["湿度の高い夏のマゾ", "催眠アプリ", "壊れたゲーム"]);
  await expect(game(page, "a1")).toContainText("購入 2026/01/02・3Djp_Art");
  await expect(game(page, "b2").locator(".note")).toHaveText("Saimin Soft");

  // Opened again, the list keeps its order.
  await page.keyboard.press("Escape");
  await page.locator('[data-button="dlsite"]').click();
  await expect(order).toHaveText("購入日の新しい順");
  await expect(game(page, "a1")).toContainText("購入 2026/01/02");
});

test("a game's menu opens its DLsite page, also from TOP", async ({ page }) => {
  mock.link.buttons[mock.link.buttons.length - 1] = dlsiteButton([
    { id: "a1", name: "湿度の高い夏のマゾ" },
  ]);
  await open(page);
  await game(page, "a1").click({ button: "right" });
  await page.getByRole("menuitem", { name: "販売ページを開く" }).click();
  await expect.poll(() => mock.link.pages).toEqual(["a1"]);
  await expect(page.getByRole("menu")).toHaveCount(0);
  await expect(dialog(page)).toBeVisible();
  // A game not known on DLsite has no page to open.
  await game(page, "c3").click({ button: "right" });
  await expect(page.getByRole("menu", { name: "壊れたゲーム" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "販売ページを開く" })).toHaveCount(0);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");

  const pinned = page.locator('[data-button="dlsite/a1"]');
  await pinned.click({ button: "right" });
  await page.getByRole("menuitem", { name: "販売ページを開く" }).click();
  await expect.poll(() => mock.link.pages).toEqual(["a1", "a1"]);
});

test("the library button's menu shows how the update goes", async ({ page }) => {
  await page.goto(`/?link=${encodeURIComponent(mock.url)}`);
  const tile = page.locator('[data-button="dlsite"]');
  await tile.click({ button: "right" });
  const menu = page.getByRole("menu", { name: "DLsite" });
  await expect(menu.getByRole("menuitem")).toHaveText(["ライブラリを更新", "更新の状況"]);
  await menu.getByRole("menuitem", { name: "更新の状況" }).click();
  const status = page.getByRole("dialog", { name: "更新の状況" });
  await expect(status).toContainText("更新の状況: DLsite");
  await expect(status).toContainText("10/28 01:35 に終わりました");
  await expect(status.getByRole("alert")).toHaveText(
    "購入一覧を読めませんでした（DLsite のログイン画面が HTTP 403 を返しました）。新しいゲームと更新は探せていません",
  );
  await page.screenshot({ path: "test-results/dlsite-round-failed.png" });

  // It follows the round while open.
  mock.link.dlsiteRound = {
    doing: "downloads",
    started: 1_793_119_000,
    finished: null,
    purchases: null,
    jobs: [
      { id: "RJ7", name: "来たゲーム", status: "完了" },
      { id: "RJ8", name: "来るゲーム", status: "ダウンロード中 40%" },
    ],
  };
  await expect(status).toContainText("ダウンロードと更新: 2 件のうち 1 件が完了");
  await expect(status.getByRole("alert")).toHaveCount(0);
  await expect(status.getByRole("listitem")).toHaveText([
    /来たゲーム\s*完了/,
    /来るゲーム\s*ダウンロード中 40%/,
  ]);
  const now = status.getByRole("button", { name: "今すぐ更新" });
  await expect(now).toBeDisabled();
  await page.screenshot({ path: "test-results/dlsite-round-running.png" });

  // Once it ends, it can be started again from here.
  mock.link.dlsiteRound = { ...mock.link.dlsiteRound, doing: null, finished: 1_793_119_600 };
  mock.link.updateAnswer = { status: 202, body: { round: true } };
  await expect(now).toBeEnabled();
  await now.click();
  await expect.poll(() => mock.link.updates).toEqual(["dlsite"]);
  await status.getByRole("button", { name: "閉じる" }).click();
  await expect(status).toHaveCount(0);
});

test("a game without a program says so on its tile", async ({ page }) => {
  await open(page);
  await game(page, "c3").click();
  await expect(game(page, "c3")).toContainText("起動できるファイルが見つかりません");
  await expect(dialog(page)).toBeVisible();
});
