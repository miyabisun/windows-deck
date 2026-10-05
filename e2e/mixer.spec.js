import { expect, test } from "@playwright/test";
import { mixerButton, muteButton, startMockLink } from "./mock-link.js";

/** @type {Awaited<ReturnType<typeof startMockLink>>} */
let mock;

test.beforeEach(async () => {
  mock = await startMockLink();
  mock.link.buttons.push(muteButton(mock.link.mixer.master), mixerButton(mock.link.mixer.master));
});

test.afterEach(async () => {
  await mock.close();
});

const open = (page) => page.goto(`/?link=${encodeURIComponent(mock.url)}`);
const tile = (page, id) => page.locator(`[data-button="${id}"]`);
const mixer = (page) => page.getByRole("dialog", { name: "ミキサー" });

test("the mute button mutes and unmutes the output", async ({ page }) => {
  await open(page);
  await expect(tile(page, "mute")).toContainText("ミュート");
  await expect(tile(page, "mute")).toContainText("音量 45%");
  await expect(tile(page, "mixer")).toContainText("音量 45%");
  await tile(page, "mute").click();
  await expect(tile(page, "mute")).toContainText("ミュート解除");
  await expect(tile(page, "mute")).toContainText("ミュート中");
  await expect(tile(page, "mixer")).toContainText("ミュート中");
  expect(mock.link.mixer.master.muted).toBe(true);
  await tile(page, "mute").click();
  await expect(tile(page, "mute")).not.toContainText("ミュート中");
  expect(mock.link.mixer.master.muted).toBe(false);
});

test("the mixer shows the whole volume first, then each app, set by tap or slide", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1920, height: 1280 });
  await open(page);
  await tile(page, "mixer").click();
  await expect(mixer(page)).toBeVisible();
  const sliders = mixer(page).getByRole("slider");
  await expect(sliders).toHaveCount(3);
  await expect(sliders.nth(0)).toHaveAccessibleName("全体");
  await expect(sliders.nth(0)).toHaveValue("45");
  await expect(sliders.nth(1)).toHaveAccessibleName("Discord");
  await expect(sliders.nth(1)).toHaveValue("100");
  await expect(sliders.nth(2)).toHaveAccessibleName("StreetFighter6");
  await expect(sliders.nth(2)).toHaveValue("20");
  await expect(mixer(page)).toContainText("45%");
  await page.screenshot({ path: "test-results/mixer.png" });

  // Each row is at least as tall as a tab, for a finger.
  const tab = (await page.getByRole("tab").first().boundingBox()).height;
  expect((await sliders.nth(1).boundingBox()).height).toBeGreaterThanOrEqual(tab);

  // A tap on the track jumps there.
  const track = await sliders.nth(2).boundingBox();
  await page.mouse.click(track.x + track.width * 0.75, track.y + track.height / 2);
  await expect.poll(() => mock.link.mixer.apps[1].volume).toBeGreaterThan(0.6);
  await expect.poll(() => mock.link.mixer.apps[1].volume).toBeLessThan(0.9);

  // Sliding moves it, and the value follows.
  const whole = await sliders.nth(0).boundingBox();
  await page.mouse.move(whole.x + whole.width * 0.45, whole.y + whole.height / 2);
  await page.mouse.down();
  await page.mouse.move(whole.x + whole.width * 0.6, whole.y + whole.height / 2, { steps: 5 });
  await page.mouse.move(whole.x + whole.width * 0.8, whole.y + whole.height / 2, { steps: 5 });
  await page.mouse.up();
  await expect.poll(() => mock.link.mixer.master.volume).toBeGreaterThan(0.7);
  await expect(sliders.nth(0)).not.toHaveValue("45");

  // The keyboard works too.
  await sliders.nth(1).focus();
  await page.keyboard.press("ArrowLeft");
  await expect.poll(() => mock.link.mixer.apps[0].volume).toBeCloseTo(0.99, 2);

  await page.keyboard.press("Escape");
  await expect(mixer(page)).toHaveCount(0);
});

test("a muted output says so in the mixer, and moving its volume unmutes it", async ({ page }) => {
  mock.link.mixer.master.muted = true;
  await open(page);
  await tile(page, "mixer").click();
  await expect(mixer(page)).toContainText("ミュート中");
  await mixer(page).getByRole("slider", { name: "全体" }).fill("30");
  await expect.poll(() => mock.link.mixer.master.volume).toBeCloseTo(0.3, 2);
  expect(mock.link.mixer.master.muted).toBe(false);
  await expect(mixer(page)).not.toContainText("ミュート中");
});

test("without apps playing sound the mixer says so", async ({ page }) => {
  mock.link.mixer.apps = [];
  await open(page);
  await tile(page, "mixer").click();
  await expect(mixer(page).getByRole("slider")).toHaveCount(1);
  await expect(mixer(page)).toContainText("音を出しているアプリはありません");
});
