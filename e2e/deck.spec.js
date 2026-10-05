import { expect, test } from "@playwright/test";
import { startMockLink } from "./mock-link.js";

/** @type {Awaited<ReturnType<typeof startMockLink>>} */
let mock;

test.beforeEach(async () => {
  mock = await startMockLink();
});

test.afterEach(async () => {
  await mock.close();
});

const open = (page) => page.goto(`/?link=${encodeURIComponent(mock.url)}`);
const tile = (page, id) => page.locator(`[data-button="${id}"]`);

test("shows the buttons in order with their current state", async ({ page }) => {
  await open(page);
  const tiles = page.locator(".tile");
  await expect(tiles).toHaveCount(2);
  await expect(tiles.nth(0)).toContainText("出力切替");
  await expect(tiles.nth(0)).toContainText("MOTU (MOTU M Series)");
  await expect(tiles.nth(0).locator("[data-icon]")).toHaveAttribute("data-icon", "speaker");
  await expect(tiles.nth(0).locator("[data-icon] svg")).toBeVisible();
  // A button without an icon of its state shows none.
  await expect(tiles.nth(1).locator("[data-icon]")).toHaveCount(0);
  await expect(tiles.nth(1)).toContainText("スト6 音量");
  await expect(tiles.nth(1)).toContainText("100%");
});

test("a press shows progress, then the new state", async ({ page }) => {
  mock.link.delayMs = 400;
  await open(page);
  const output = tile(page, "output");
  await output.click();
  await expect(output).toHaveAttribute("aria-busy", "true");
  await expect(output.locator(".busy")).toBeVisible();
  // A second tap while busy is ignored.
  await output.click({ force: true });
  await expect(output).toContainText("BTイヤホン (JBL Tour Pro 3)");
  await expect(output).toHaveAttribute("aria-busy", "false");
  // The icon follows the device: headphones now, a speaker before.
  await expect(output.locator("[data-icon]")).toHaveAttribute("data-icon", "headphones");
  expect(mock.link.presses).toEqual(["output"]);

  await tile(page, "sf6-volume").click();
  await expect(tile(page, "sf6-volume")).toContainText("20%");
});

test("a failed press says why on that button until it is pressed again", async ({ page }) => {
  await open(page);
  mock.link.nextFailure = {
    status: 409,
    body: { error: "device_unavailable", message: "jbl is not connected" },
  };
  const output = tile(page, "output");
  await output.click();
  await expect(output).toContainText("切替先のデバイスがつながっていません");
  await expect(output).toHaveClass(/failed/);
  await expect(output).toContainText("MOTU (MOTU M Series)");
  await expect(tile(page, "sf6-volume")).not.toHaveClass(/failed/);
  await page.screenshot({ path: "test-results/deck-failure.png" });

  await output.click();
  await expect(output).not.toContainText("つながっていません");
  await expect(output).toContainText("BTイヤホン (JBL Tour Pro 3)");
});

test("changes made outside the panel show up", async ({ page }) => {
  await open(page);
  await expect(tile(page, "sf6-volume")).toContainText("100%");
  mock.change(mock.volume(0.35));
  await expect(tile(page, "sf6-volume")).toContainText("35%");
  mock.change({
    ...mock.volume(0),
    state: { kind: "volume", running: false, volume: null, levels: [0.2, 1] },
  });
  await expect(tile(page, "sf6-volume")).toContainText("起動していません");
});

test("losing windows-link disables the buttons until it is back", async ({ page }) => {
  await open(page);
  await expect(page.locator(".tile")).toHaveCount(2);
  mock.goDown();
  const banner = page.getByRole("status").filter({ hasText: "接続できません" });
  await expect(banner).toBeVisible();
  await expect(banner).toContainText(mock.url);
  await expect(tile(page, "output")).toHaveAttribute("aria-disabled", "true");
  await page.screenshot({ path: "test-results/deck-disconnected.png" });
  await tile(page, "output").click({ force: true });
  expect(mock.link.presses).toEqual([]);

  mock.comeUp();
  await expect(banner).toBeHidden({ timeout: 10_000 });
  await expect(tile(page, "output")).toHaveAttribute("aria-disabled", "false");
  await tile(page, "output").click();
  await expect(tile(page, "output")).toContainText("BTイヤホン (JBL Tour Pro 3)");
});

test("says how to add buttons when there are none", async ({ page }) => {
  mock.link.buttons = [];
  await open(page);
  await expect(page.getByText("ボタンがありません。")).toBeVisible();
  await expect(page.getByText("config.yaml")).toBeVisible();
});

test("works from the keyboard", async ({ page }) => {
  mock.link.desktops = [];
  await open(page);
  await expect(page.locator(".tile")).toHaveCount(2);
  // The tab bar comes first: create a desktop, then sleep, then the buttons.
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "デスクトップを作る" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "スリープ" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(tile(page, "output")).toBeFocused();
  const ring = await tile(page, "output").evaluate((el) => getComputedStyle(el).outlineWidth);
  expect(ring).toBe("2px");
  await page.keyboard.press("Enter");
  await expect(tile(page, "output")).toContainText("BTイヤホン (JBL Tour Pro 3)");
});

for (const [scheme, background] of [
  ["dark", "rgb(25, 25, 25)"],
  ["light", "rgb(250, 246, 239)"],
]) {
  for (const [width, height, columns] of [
    [1920, 1080, 7],
    [1280, 800, 5],
  ]) {
    test(`${scheme} ${width}x${height}: ${columns} columns, no sideways scroll`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await page.setViewportSize({ width, height });
      await open(page);
      await expect(page.locator(".tile")).toHaveCount(2);
      const layout = await page.evaluate(() => ({
        background: getComputedStyle(document.body).backgroundColor,
        columns: getComputedStyle(document.querySelector(".grid")).gridTemplateColumns.split(" ")
          .length,
        overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
        tileHeight: document.querySelector(".tile").getBoundingClientRect().height,
      }));
      expect(layout.background).toBe(background);
      expect(layout.columns).toBe(columns);
      expect(layout.overflow).toBe(0);
      expect(layout.tileHeight).toBeGreaterThanOrEqual(144);
      await page.screenshot({ path: `test-results/deck-${scheme}-${width}x${height}.png` });
    });
  }
}

test.describe("on a touch screen", () => {
  test.use({ hasTouch: true });

  test("a tap does not leave the hover highlight, a mouse hover shows it", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "dark" });
    await open(page);
    const output = tile(page, "output");
    const background = () => output.evaluate((el) => getComputedStyle(el).backgroundColor);
    await output.tap();
    await expect(output).toContainText("BTイヤホン (JBL Tour Pro 3)");
    expect(await background()).toBe("rgb(35, 35, 35)");

    await tile(page, "sf6-volume").hover();
    await expect(tile(page, "sf6-volume")).toHaveClass(/mouse/);
    expect(
      await tile(page, "sf6-volume").evaluate((el) => getComputedStyle(el).backgroundColor),
    ).toBe("rgb(44, 44, 44)");
  });
});

test("a Discord voice button shows and toggles whether you are in its channel", async ({
  page,
}) => {
  const voice = (state) => ({
    id: "vc-uf4",
    type: "discord.voice",
    label: "UF4フレンド",
    desktop: null,
    state: { kind: "voice", reason: null, ...state },
  });
  mock.link.buttons = [voice({ available: true, joined: false })];
  await open(page);
  const tile = page.locator('[data-button="vc-uf4"]');
  await expect(tile).toContainText("ボイチャに入室");
  await tile.click();
  await expect(tile).toContainText("ボイチャから退室");
  await expect(tile).toContainText("参加中");

  mock.change(voice({ available: false, joined: false, reason: "Discord is not running" }));
  await expect(tile).toContainText("Discord 未接続");
  await expect(tile).toContainText("押すと起動して参加します");
  await expect(tile).toHaveAttribute("aria-disabled", "false");
  await page.screenshot({ path: "test-results/deck-voice.png" });
});
