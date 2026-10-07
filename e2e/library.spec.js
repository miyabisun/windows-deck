import { expect, test } from "@playwright/test";
import { dlsiteButton, libraryButton, startMockLink } from "./mock-link.js";

/** @type {Awaited<ReturnType<typeof startMockLink>>} */
let mock;

test.beforeEach(async () => {
  mock = await startMockLink();
  mock.link.buttons.push(libraryButton());
});

test.afterEach(async () => {
  await mock.close();
});

const open = (page) => page.goto(`/?link=${encodeURIComponent(mock.url)}`);
const dialog = (page) => page.getByRole("dialog", { name: "Steam" });
const game = (page, id) => dialog(page).locator(`[data-button="${id}"]`);
const names = (page) => dialog(page).locator(".tile .label").allTextContents();
const chip = (page, name) => dialog(page).getByRole("button", { name, exact: true });

/** The list starts with only the unlabeled games; show every game instead. */
async function showAll(page) {
  await chip(page, "ラベル非登録").click();
  await expect(chip(page, "ラベル非登録")).toHaveAttribute("aria-pressed", "false");
}

/** Every row is at least as tall as a desktop tab, for a finger. */
async function expectTabTall(page, rows) {
  const tab = (await page.getByRole("tab").first().boundingBox()).height;
  expect(rows.length).toBeGreaterThan(0);
  for (const row of rows) expect((await row.boundingBox()).height).toBeGreaterThanOrEqual(tab);
}

/** Hold the mouse down on a locator long enough for a long press. */
async function longPress(page, locator) {
  const box = await locator.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.waitForTimeout(700);
  await page.mouse.up();
}

test("the library opens at 80% of the screen with the search field focused", async ({ page }) => {
  await page.setViewportSize({ width: 1920, height: 1280 });
  await open(page);
  await page.locator('[data-button="games"]').click();
  await expect(dialog(page)).toBeVisible();
  const box = await dialog(page).boundingBox();
  expect(Math.round(box.width)).toBe(1536);
  expect(Math.round(box.height)).toBe(1024);
  await expect(page.getByRole("searchbox", { name: "名前で探す" })).toBeFocused();
  // It starts with the games that have no label yet: 「ラベル非登録」 is first and on.
  await expect(dialog(page).locator(".chip").first()).toHaveText("ラベル非登録");
  await expect(chip(page, "ラベル非登録")).toHaveAttribute("aria-pressed", "true");
  await expect.poll(() => names(page)).toEqual(["Slay the Spire"]);
  await showAll(page);
  // 非表示 is hidden until its label is selected.
  await expect
    .poll(() => names(page))
    .toEqual(["Street Fighter™ 6", "Slay the Spire", "Slay the Spire 2"]);
  await expect(game(page, "2868840")).toContainText("未インストール");
  for (const one of await dialog(page).locator(".chip").all()) {
    const size = await one.boundingBox();
    expect(size.height).toBeGreaterThanOrEqual(48);
  }
  await page.screenshot({ path: "test-results/library-open.png" });

  await page.keyboard.press("Escape");
  await expect(dialog(page)).toHaveCount(0);
});

test("ラベル非登録 and the other labels are not selected together", async ({ page }) => {
  await open(page);
  await page.locator('[data-button="games"]').click();
  const chip = (name) => dialog(page).getByRole("button", { name, exact: true });
  await expect(chip("ラベル非登録")).toHaveAttribute("aria-pressed", "true");
  await chip("R15").click();
  await expect(chip("R15")).toHaveAttribute("aria-pressed", "true");
  await expect(chip("ラベル非登録")).toHaveAttribute("aria-pressed", "false");
  await expect.poll(() => names(page)).toEqual(["Slay the Spire 2"]);
  await chip("ラベル非登録").click();
  await expect(chip("ラベル非登録")).toHaveAttribute("aria-pressed", "true");
  await expect(chip("R15")).toHaveAttribute("aria-pressed", "false");
  await expect.poll(() => names(page)).toEqual(["Slay the Spire"]);
});

test("typing and labels narrow the list; 非表示 shows only while selected", async ({ page }) => {
  await open(page);
  await page.locator('[data-button="games"]').click();
  await showAll(page);
  await page.getByRole("searchbox").fill("spire");
  await expect.poll(() => names(page)).toEqual(["Slay the Spire", "Slay the Spire 2"]);

  const chip = (name) => dialog(page).getByRole("button", { name, exact: true });
  await chip("R15").click();
  await expect(chip("R15")).toHaveAttribute("aria-pressed", "true");
  await expect.poll(() => names(page)).toEqual(["Slay the Spire 2"]);

  await page.getByRole("searchbox").fill("");
  await chip("R15").click();
  await expect(chip("R15")).toHaveAttribute("aria-pressed", "false");
  await chip("非表示").click();
  await expect.poll(() => names(page)).toEqual(["METAL SLUG"]);
  await page.getByRole("searchbox").fill("zelda");
  await expect(dialog(page)).toContainText("見つかりません");
});

test("tapping a game starts it and leaves the list as it was", async ({ page }) => {
  await page.setViewportSize({ width: 1920, height: 1280 });
  mock.link.library.items = Array.from({ length: 60 }, (_, i) => ({
    id: String(1000 + i),
    name: `Game ${i}`,
    installed: true,
    labels: [],
  }));
  await open(page);
  await page.locator('[data-button="games"]').click();
  await game(page, "1050").scrollIntoViewIfNeeded();
  const scroll = dialog(page).locator(".scroll");
  const before = await scroll.evaluate((el) => el.scrollTop);
  expect(before).toBeGreaterThan(0);
  await game(page, "1050").click();
  await expect(dialog(page).getByRole("status").last()).toHaveText("「Game 50」を起動しました");
  expect(mock.link.started).toEqual(["1050"]);
  // Still open where it was, to pick the next game or its label.
  await expect(dialog(page)).toBeVisible();
  expect(await scroll.evaluate((el) => el.scrollTop)).toBe(before);
  await page.keyboard.press("Escape");
  await expect(dialog(page)).toHaveCount(0);

  // Tapping outside the list closes it too.
  await page.locator('[data-button="games"]').click();
  await expect(dialog(page)).toBeVisible();
  await page.mouse.click(5, 5);
  await expect(dialog(page)).toHaveCount(0);
});

const menuItem = (page, name) => page.getByRole("menuitem", { name, exact: true });

test("a game's menu pins it to the tab, where its menu takes it off", async ({ page }) => {
  await open(page);
  await page.locator('[data-button="games"]').click();
  await longPress(page, game(page, "646570"));
  await expect(page.getByRole("menu", { name: "Slay the Spire" })).toBeVisible();
  await expectTabTall(page, await page.getByRole("menuitem").all());
  await page.screenshot({ path: "test-results/library-game-menu.png" });
  await menuItem(page, "TOPに固定").click();
  await expect(dialog(page).getByRole("status")).toHaveText(
    "「Slay the Spire」を TOP に固定しました",
  );
  await expect(game(page, "646570")).toContainText("TOP に固定中");
  // The long press does not also start the game.
  expect(mock.link.started).toEqual([]);
  await page.getByRole("button", { name: "閉じる" }).click();

  const pinned = page.locator('[data-button="games/646570"]');
  await expect(pinned).toContainText("Slay the Spire");
  await expect(pinned.locator("img.cover")).toBeVisible();
  await page.screenshot({ path: "test-results/library-pinned.png" });
  await pinned.click();
  await expect.poll(() => mock.link.started).toEqual(["646570"]);

  await longPress(page, pinned);
  await menuItem(page, "TOPから外す").click();
  await expect(pinned).toHaveCount(0);
  expect(mock.link.started).toEqual(["646570"]);
});

test("a game's menu opens its folder, only when it is installed", async ({ page }) => {
  await open(page);
  await page.locator('[data-button="games"]').click();
  await longPress(page, game(page, "646570"));
  await menuItem(page, "ローカルファイル閲覧").click();
  await expect.poll(() => mock.link.folders).toEqual(["646570"]);
  await showAll(page);
  await longPress(page, game(page, "2868840"));
  await expect(menuItem(page, "ローカルファイル閲覧")).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);
  await expect(dialog(page)).toBeVisible();
});

test("a game's labels are set from its menu, including a new one", async ({ page }) => {
  await open(page);
  await page.locator('[data-button="games"]').click();
  await longPress(page, game(page, "646570"));
  await menuItem(page, "ラベル設定").click();
  const picker = page.getByRole("dialog", { name: "ラベル設定" });
  await expect(picker).toContainText("ラベル設定: Slay the Spire");
  const box = (name) => picker.getByRole("checkbox", { name });
  await expect(box("非表示")).toHaveAttribute("aria-checked", "false");
  await expectTabTall(page, [
    ...(await picker.getByRole("checkbox").all()),
    picker.getByRole("button", { name: "新しいラベル" }),
  ]);
  await box("非表示").click();
  await expect(box("非表示")).toHaveAttribute("aria-checked", "true");
  expect(mock.link.library.items[1].labels).toEqual(["hidden"]);
  await page.screenshot({ path: "test-results/library-label-picker.png" });

  await picker.getByRole("button", { name: "新しいラベル" }).click();
  await page.getByRole("textbox", { name: "新しいラベル" }).fill("ローグライク");
  await page.getByRole("button", { name: "作って付ける" }).click();
  await expect(box("ローグライク")).toHaveAttribute("aria-checked", "true");
  await picker.getByRole("button", { name: "閉じる" }).click();
  await expect(picker).toHaveCount(0);
  // The list follows: the game is now hidden, and the new label has a chip.
  await expect.poll(() => names(page)).not.toContain("Slay the Spire");
  await expect(
    dialog(page).getByRole("button", { name: "ローグライク", exact: true }),
  ).toBeVisible();
});

test("the + chip makes a label, and a label's menu renames or deletes it", async ({ page }) => {
  await open(page);
  await page.locator('[data-button="games"]').click();
  await dialog(page).getByRole("button", { name: "ラベルを作る" }).click();
  await page.getByRole("textbox", { name: "新しいラベル" }).fill("RPG");
  await page.getByRole("button", { name: "作る", exact: true }).click();
  const chip = (name) => dialog(page).getByRole("button", { name, exact: true });
  await expect(chip("RPG")).toBeVisible();

  await longPress(page, chip("RPG"));
  await menuItem(page, "名前変更").click();
  const rename = page.getByRole("textbox", { name: "ラベルの名前変更" });
  await expect(rename).toHaveValue("RPG");
  await rename.fill("JRPG");
  await page.getByRole("button", { name: "変更する" }).click();
  await expect(chip("JRPG")).toBeVisible();

  await longPress(page, chip("JRPG"));
  await menuItem(page, "削除").click();
  await expect(page.getByRole("dialog", { name: "「JRPG」を削除しますか？" })).toContainText(
    "ゲームはライブラリに残ります",
  );
  await page.getByRole("button", { name: "削除する" }).click();
  await expect(chip("JRPG")).toHaveCount(0);

  // Steam's own labels cannot be renamed or deleted, and a long press does not toggle.
  await longPress(page, chip("非表示"));
  await expect(menuItem(page, "名前変更")).toBeDisabled();
  await expect(menuItem(page, "削除")).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(chip("非表示")).toHaveAttribute("aria-pressed", "false");
});

test("while labels cannot be changed, the panel says why", async ({ page }) => {
  mock.link.library.labels_locked = "Steam is not running";
  await open(page);
  await page.locator('[data-button="games"]').click();
  await expect(dialog(page).getByRole("button", { name: "ラベルを作る" })).toBeDisabled();
  await longPress(page, game(page, "646570"));
  await menuItem(page, "ラベル設定").click();
  const picker = page.getByRole("dialog", { name: "ラベル設定" });
  await expect(picker).toContainText("Steam が起動していないため");
  await expect(picker.getByRole("checkbox", { name: "非表示" })).toBeDisabled();
});

test("a right click opens the same menus as a long press", async ({ page }) => {
  await open(page);
  await page.locator('[data-button="games"]').click();
  await game(page, "646570").click({ button: "right" });
  await expect(page.getByRole("menu", { name: "Slay the Spire" })).toBeVisible();
  await page.getByRole("menuitem", { name: "TOPに固定", exact: true }).click();
  expect(mock.link.started).toEqual([]);
  await chip(page, "R15").click({ button: "right" });
  await expect(page.getByRole("menu", { name: "R15" })).toBeVisible();
  await page.keyboard.press("Escape");
  // A right click neither toggles a label nor starts a game.
  await expect(chip(page, "R15")).toHaveAttribute("aria-pressed", "false");
  await page.getByRole("button", { name: "閉じる" }).click();

  const pinned = page.locator('[data-button="games/646570"]');
  await pinned.click({ button: "right" });
  await expect(page.getByRole("menuitem", { name: "TOPから外す", exact: true })).toBeVisible();
  expect(mock.link.started).toEqual([]);
});

test("Steam games have no license keys to show", async ({ page }) => {
  await open(page);
  await page.locator('[data-button="games"]').click();
  await longPress(page, game(page, "646570"));
  await expect(page.getByRole("menuitem", { name: "TOPに固定" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "シリアル番号" })).toHaveCount(0);
});

test("dragging a game onto a label puts it there", async ({ page }) => {
  await open(page);
  await page.locator('[data-button="games"]').click();
  await game(page, "646570").dragTo(chip(page, "R15"));
  await expect(dialog(page).getByRole("status")).toHaveText(
    "「Slay the Spire」に「R15」を付けました",
  );
  expect(mock.link.library.items[1].labels).toEqual(["uc-r15"]);
  // It had no label, so the list that starts with the unlabeled games lets it go.
  await expect(dialog(page)).toContainText("見つかりません");
  expect(mock.link.started).toEqual([]);

  // A game that already has the label stays as it is.
  await showAll(page);
  await game(page, "2868840").dragTo(chip(page, "R15"));
  await expect(dialog(page).getByRole("status")).toHaveText(
    "「Slay the Spire 2」にはもう「R15」が付いています",
  );
  await game(page, "1364780").dragTo(chip(page, "非表示"));
  await expect.poll(() => names(page)).not.toContain("Street Fighter™ 6");
});

test("while labels cannot be changed, a game cannot be dropped on a label", async ({ page }) => {
  mock.link.library.labels_locked = "Steam is not running";
  await open(page);
  await page.locator('[data-button="games"]').click();
  await game(page, "646570").dragTo(chip(page, "R15"));
  expect(mock.link.library.items[1].labels).toEqual([]);
  await expect(game(page, "646570")).toBeVisible();
});

test("a library listing only installed games says why", async ({ page }) => {
  mock.link.library.partial = "secrets.yaml has no steam api_key";
  await open(page);
  await page.locator('[data-button="games"]').click();
  await expect(dialog(page)).toContainText("Steam の API キーが無いため");
});

test("with more games than fit, the list scrolls and each tile shows its whole name", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1920, height: 1280 });
  mock.link.library.items = Array.from({ length: 60 }, (_, i) => ({
    id: String(1000 + i),
    name: `Game ${i}`,
    installed: true,
    labels: [],
  }));
  await open(page);
  await page.locator('[data-button="games"]').click();
  const tile = game(page, "1000");
  const fits = await tile.evaluate((el) => {
    const box = el.getBoundingClientRect();
    const label = el.querySelector(".label").getBoundingClientRect();
    return label.bottom <= box.bottom;
  });
  expect(fits).toBe(true);
  await game(page, "1059").scrollIntoViewIfNeeded();
  await expect(game(page, "1059")).toBeInViewport();
});

test("a shop that needs signing in says so on its button and in its list", async ({ page }) => {
  await page.setViewportSize({ width: 1920, height: 1280 });
  mock.link.buttons[mock.link.buttons.length - 1].state.sign_in = "fanza";
  mock.link.library.sign_in = "fanza";
  await open(page);
  const tile = page.locator('[data-button="games"]');
  await expect(tile).toContainText("ログインが必要です");
  await tile.click();
  const banner = dialog(page).getByRole("alert");
  await expect(banner).toContainText("FANZA にログインしていません");
  const signIn = dialog(page).getByRole("button", { name: "ログイン", exact: true });
  const size = await signIn.boundingBox();
  expect(size.height).toBeGreaterThanOrEqual(48);
  await page.screenshot({ path: "test-results/library-sign-in.png" });
  // Only the panel's app has a login window.
  await signIn.click();
  await expect(dialog(page).getByRole("status")).toHaveText(
    "ログインはタッチパネルのアプリからだけできます",
  );
});

test("a library button's menu updates the library and says what started", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await open(page);
  const tile = page.locator('[data-button="games"]');
  await expect(tile).toContainText(/Steam\s*ライブラリ/);
  await tile.click({ button: "right" });
  const menu = page.getByRole("menu", { name: "Steam" });
  await expect(menu).toBeVisible();
  // A right click does not open the library (the menu is a dialog of the same name).
  const search = page.getByRole("searchbox", { name: "名前で探す" });
  await expect(search).toHaveCount(0);
  await expectTabTall(page, [page.getByRole("menuitem", { name: "ライブラリを更新" })]);
  await page.screenshot({ path: "test-results/library-update-menu.png" });
  await page.getByRole("menuitem", { name: "ライブラリを更新" }).click();
  await expect(menu).toHaveCount(0);
  const toast = page.getByRole("status").filter({ hasText: "アップデート" });
  await expect(toast).toHaveText(
    "アップデート 3 件を始めました。インストール 24 件は Steam の画面で確定してください",
  );
  expect(mock.link.updates).toEqual(["games"]);
  await page.screenshot({ path: "test-results/library-update-toast.png" });
  // It goes after a few seconds.
  await expect(toast).toHaveCount(0, { timeout: 5000 });

  // A long press opens the same menu, not the library.
  await longPress(page, tile);
  await expect(menu).toBeVisible();
  await expect(search).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
});

test("a library that cannot be updated says why on its button until it is opened", async ({
  page,
}) => {
  mock.link.updateAnswer = {
    status: 409,
    body: { error: "update_unavailable", message: "Steam is not running" },
  };
  await open(page);
  const tile = page.locator('[data-button="games"]');
  await tile.click({ button: "right" });
  await page.getByRole("menuitem", { name: "ライブラリを更新" }).click();
  await expect(tile).toContainText("Steam が起動していないため、ライブラリを更新できません");
  await expect(tile).toHaveClass(/failed/);
  await tile.click();
  await expect(dialog(page)).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(tile).not.toContainText("ライブラリを更新できません");
});

test("a shop's update starts its downloads", async ({ page }) => {
  mock.link.buttons.push(dlsiteButton());
  mock.link.updateAnswer = { status: 202, body: { round: true } };
  await open(page);
  const tile = page.locator('[data-button="dlsite"]');
  await expect(tile).toContainText(/DLsite\s*ライブラリ/);
  await tile.click({ button: "right" });
  await page.getByRole("menu", { name: "DLsite" }).getByRole("menuitem").click();
  await expect(page.getByRole("status").filter({ hasText: "ダウンロード" })).toHaveText(
    "ダウンロードと更新を始めました（進み具合はライブラリに出ます）",
  );
  expect(mock.link.updates).toEqual(["dlsite"]);
});
