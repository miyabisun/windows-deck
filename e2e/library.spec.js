import { expect, test } from "@playwright/test";
import { libraryButton, startMockLink } from "./mock-link.js";

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
const dialog = (page) => page.getByRole("dialog", { name: "ゲーム検索" });
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

test("the library opens at 80% of the screen with the search field focused", async ({ page }) => {
  await page.setViewportSize({ width: 1920, height: 1280 });
  await open(page);
  await page.locator('[data-button="games"]').click();
  await expect(dialog(page)).toBeVisible();
  const box = await dialog(page).boundingBox();
  expect(Math.round(box.width)).toBe(1536);
  expect(Math.round(box.height)).toBe(1024);
  await expect(page.getByRole("searchbox", { name: "名前で探す" })).toBeFocused();
  // outdate is hidden until its label is selected.
  await expect
    .poll(() => names(page))
    .toEqual(["Street Fighter™ 6", "Slay the Spire", "Slay the Spire 2"]);
  await expect(game(page, "2868840")).toContainText("未インストール");
  for (const chip of await dialog(page).locator(".chip").all()) {
    const size = await chip.boundingBox();
    expect(size.height).toBeGreaterThanOrEqual(48);
  }
  await page.screenshot({ path: "test-results/library-open.png" });

  await page.keyboard.press("Escape");
  await expect(dialog(page)).toHaveCount(0);
});

test("typing and labels narrow the list; outdate shows only while selected", async ({ page }) => {
  await open(page);
  await page.locator('[data-button="games"]').click();
  await page.keyboard.type("spire");
  await expect.poll(() => names(page)).toEqual(["Slay the Spire", "Slay the Spire 2"]);

  const chip = (name) => dialog(page).getByRole("button", { name, exact: true });
  await chip("R15").click();
  await expect(chip("R15")).toHaveAttribute("aria-pressed", "true");
  await expect.poll(() => names(page)).toEqual(["Slay the Spire 2"]);

  await page.getByRole("searchbox").fill("");
  await chip("R15").click();
  await expect(chip("R15")).toHaveAttribute("aria-pressed", "false");
  await chip("outdate").click();
  await expect.poll(() => names(page)).toEqual(["METAL SLUG"]);
  await page.getByRole("searchbox").fill("zelda");
  await expect(dialog(page)).toContainText("見つかりません");
});

test("tapping a game starts it and closes the list", async ({ page }) => {
  await open(page);
  await page.locator('[data-button="games"]').click();
  await game(page, "1364780").click();
  await expect(dialog(page)).toHaveCount(0);
  expect(mock.link.started).toEqual(["1364780"]);

  // Tapping outside the list closes it too.
  await page.locator('[data-button="games"]').click();
  await expect(dialog(page)).toBeVisible();
  await page.mouse.click(5, 5);
  await expect(dialog(page)).toHaveCount(0);
});

test("a long press pins a game to the tab, where a long press takes it off", async ({ page }) => {
  await open(page);
  await page.locator('[data-button="games"]').click();
  await longPress(page, game(page, "646570"));
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
  await expect(pinned).toHaveCount(0);
  expect(mock.link.started).toEqual(["646570"]);
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
