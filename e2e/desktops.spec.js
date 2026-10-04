import { expect, test } from "@playwright/test";
import { initialDesktops, startMockLink } from "./mock-link.js";

/** @type {Awaited<ReturnType<typeof startMockLink>>} */
let mock;

test.beforeEach(async () => {
  mock = await startMockLink();
});

test.afterEach(async () => {
  await mock.close();
});

const open = (page, extra = "") => page.goto(`/?link=${encodeURIComponent(mock.url)}${extra}`);
const tabs = (page) => page.getByRole("tab");
const selected = (page) => page.locator('[role="tab"][aria-selected="true"]');

test("shows the desktops as tabs in order with the current one selected", async ({ page }) => {
  mock.link.desktops = initialDesktops(1);
  await open(page);
  await expect(tabs(page)).toHaveText(["dev", "ゲーム", "ブルアカ", "アダルト"]);
  await expect(selected(page)).toHaveText("ゲーム");
});

test("follows desktop switches made in Windows", async ({ page }) => {
  await open(page);
  await expect(selected(page)).toHaveText("dev");
  mock.setDesktops(initialDesktops(2));
  await expect(selected(page)).toHaveText("ブルアカ");
  mock.setDesktops(initialDesktops(0));
  await expect(selected(page)).toHaveText("dev");
});

test("tapping a tab asks windows-link to switch", async ({ page }) => {
  await open(page);
  await page.getByRole("tab", { name: "アダルト" }).click();
  await expect(selected(page)).toHaveText("アダルト");
  expect(mock.link.switches).toEqual(["GUID-3"]);
  // The current tab does not send anything.
  await page.getByRole("tab", { name: "アダルト" }).click();
  expect(mock.link.switches).toEqual(["GUID-3"]);
});

test("renamed, added and removed desktops update the tabs", async ({ page }) => {
  await open(page);
  await expect(tabs(page)).toHaveCount(4);
  const renamed = initialDesktops(0).map((d) => (d.index === 0 ? { ...d, name: "開発" } : d));
  mock.setDesktops(renamed, "renamed");
  await expect(tabs(page).first()).toHaveText("開発");

  const added = [...renamed, { id: "GUID-4", name: "デスクトップ 5", index: 4, current: false }];
  mock.setDesktops(added, "created");
  await expect(tabs(page)).toHaveText(["開発", "ゲーム", "ブルアカ", "アダルト", "デスクトップ 5"]);

  mock.setDesktops(
    renamed.filter((d) => d.index !== 2),
    "removed",
  );
  await expect(tabs(page)).toHaveText(["開発", "ゲーム", "アダルト"]);
});

test("each tab shows its own buttons and the shared ones", async ({ page }) => {
  mock.link.buttons = mock.link.buttons.map((b) =>
    b.id === "sf6-volume" ? { ...b, desktop: "ゲーム" } : b,
  );
  await open(page);
  const tiles = page.locator(".tile");
  await expect(tiles).toHaveCount(1);
  await expect(tiles.first()).toContainText("出力切替");

  await page.getByRole("tab", { name: "ゲーム" }).click();
  await expect(tiles).toHaveCount(2);
  await expect(tiles.nth(1)).toContainText("スト6 音量");

  mock.link.buttons = mock.link.buttons.map((b) => ({ ...b, desktop: "アダルト" }));
  await page.reload();
  await expect(page.getByText("このデスクトップに割り当てたボタンはありません。")).toBeVisible();
});

test("without virtual desktops there are no tabs and every button shows", async ({ page }) => {
  mock.link.desktops = [];
  mock.link.desktopsError = "virtual desktops are not enabled";
  mock.link.buttons = mock.link.buttons.map((b) =>
    b.id === "sf6-volume" ? { ...b, desktop: "ゲーム" } : b,
  );
  await open(page);
  await expect(page.locator(".tile")).toHaveCount(2);
  await expect(tabs(page)).toHaveCount(0);
});

test("a failed switch says why", async ({ page }) => {
  await open(page);
  // Windows removed the desktop, but the panel has not heard yet.
  mock.link.desktops = mock.link.desktops.filter((d) => d.id !== "GUID-3");
  await page.getByRole("tab", { name: "アダルト" }).click();
  await expect(page.getByRole("alert")).toHaveText("このデスクトップはもうありません");
  await expect(selected(page)).toHaveText("dev");
});

test("pins its window on every connection", async ({ page }) => {
  await open(page, "&hwnd=4723016");
  await expect.poll(() => mock.link.pins).toEqual(["4723016"]);
  mock.goDown();
  await expect(page.getByRole("status").filter({ hasText: "接続できません" })).toBeVisible();
  mock.comeUp();
  await expect.poll(() => mock.link.pins, { timeout: 10_000 }).toEqual(["4723016", "4723016"]);
  // Explorer restarted: windows-link reconnects to it and says so.
  mock.setDesktops(initialDesktops(0), "reconnected");
  await expect.poll(() => mock.link.pins).toEqual(["4723016", "4723016", "4723016"]);
});

test("the tab bar fits a narrow panel and marks the selection", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.emulateMedia({ colorScheme: "dark" });
  await open(page);
  await expect(selected(page)).toHaveText("dev");
  const bar = await page.evaluate(() => {
    const tab = document.querySelector('[role="tab"][aria-selected="true"]');
    const style = getComputedStyle(tab);
    return {
      height: tab.getBoundingClientRect().height,
      marker: style.boxShadow,
      overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    };
  });
  // Half a button: 144 / 2.
  expect(bar.height).toBe(72);
  expect(bar.marker).toContain("rgb(76, 195, 203)");
  expect(bar.overflow).toBe(0);
  await page.screenshot({ path: "test-results/deck-tabs-dark-1280x800.png" });
  await page.emulateMedia({ colorScheme: "light" });
  await page.screenshot({ path: "test-results/deck-tabs-light-1280x800.png" });
});

test("tabs work from the keyboard", async ({ page }) => {
  await open(page);
  await expect(selected(page)).toHaveText("dev");
  await page.keyboard.press("Tab");
  await expect(page.getByRole("tab", { name: "dev" })).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("tab", { name: "ゲーム" })).toBeFocused();
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.press("ArrowLeft");
  await expect(page.getByRole("tab", { name: "アダルト" })).toBeFocused();
  expect(mock.link.switches).toEqual([]);
  await page.keyboard.press("Enter");
  await expect(selected(page)).toHaveText("アダルト");
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "デスクトップを作る" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "スリープ" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.locator('[data-button="output"]')).toBeFocused();
});

test("shared buttons skip the desktops they except", async ({ page }) => {
  mock.link.buttons = mock.link.buttons.map((b) =>
    b.id === "output" ? { ...b, except: ["dev"] } : b,
  );
  await open(page);
  await expect(selected(page)).toHaveText("dev");
  await expect(page.locator('[data-button="output"]')).toHaveCount(0);
  await page.getByRole("tab", { name: "ゲーム" }).click();
  await expect(page.locator('[data-button="output"]')).toHaveCount(1);
});

test("the + button creates a desktop for a desktop file that has none", async ({ page }) => {
  mock.link.unmatched = ["SF6"];
  await open(page);
  await page.getByRole("button", { name: "デスクトップを作る" }).click();
  await page.getByRole("menuitem", { name: "SF6" }).click();
  await expect(selected(page)).toHaveText("SF6");
  await page.getByRole("button", { name: "デスクトップを作る" }).click();
  await expect(page.getByRole("menu")).toContainText("作れるデスクトップはありません");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);
});

test("the moon button puts the PC to sleep", async ({ page }) => {
  await open(page);
  await expect(selected(page)).toHaveText("dev");
  await page.getByRole("button", { name: "スリープ" }).click();
  await expect.poll(() => mock.link.sleeps).toBe(1);
});

test("buttons with an icon show it", async ({ page }) => {
  mock.link.buttons = mock.link.buttons.map((b) => (b.id === "output" ? { ...b, icon: true } : b));
  await open(page);
  const picture = page.locator('[data-button="output"] img');
  await expect(picture).toHaveAttribute("src", `${mock.url}/buttons/output/icon`);
  await expect.poll(() => picture.evaluate((img) => img.naturalWidth)).toBe(64);
  await expect(page.locator('[data-button="sf6-volume"] img')).toHaveCount(0);
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.screenshot({ path: "test-results/deck-tabs-actions.png" });
});
