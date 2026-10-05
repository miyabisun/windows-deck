import { expect, test } from "@playwright/test";
import { dlsiteButton, startMockLink } from "./mock-link.js";

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
  await expect(dialog(page)).toHaveCount(0);
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
  // The choice starts the game and closes the list.
  await expect(chooser).toHaveCount(0);
  await expect(dialog(page)).toHaveCount(0);
  expect(mock.link.started).toEqual(["b2:startup.exe"]);

  // Next time it starts at once.
  await page.locator('[data-button="dlsite"]').click();
  await game(page, "b2").click();
  await expect(dialog(page)).toHaveCount(0);
  expect(mock.link.started).toEqual(["b2:startup.exe", "b2:startup.exe"]);
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

test("a game without a program says so on its tile", async ({ page }) => {
  await open(page);
  await game(page, "c3").click();
  await expect(game(page, "c3")).toContainText("起動できるファイルが見つかりません");
  await expect(dialog(page)).toBeVisible();
});
