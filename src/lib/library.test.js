import { describe, expect, it } from "vitest";
import {
  UNLABELED,
  labelsLockedReason,
  partialReason,
  pictureUrl,
  pinsOf,
  visibleItems,
} from "./library.js";

const item = (id, name, labels = []) => ({ id, name, labels, installed: true });

const items = [
  item("1", "Street Fighter™ 6"),
  item("2", "Slay the Spire", ["お気に入り"]),
  item("3", "Slay the Spire 2", ["R15"]),
  item("4", "METAL SLUG", ["outdate"]),
  item("5", "Ｒｅｍｎａｎｔ", ["R15", "outdate"]),
];
const ids = (list) => list.map((i) => i.id);

describe("visibleItems", () => {
  it("lists everything except the hidden labels without a query or label", () => {
    expect(ids(visibleItems(items, { hide: ["outdate"] }))).toEqual(["1", "2", "3"]);
    expect(ids(visibleItems(items, {}))).toEqual(["1", "2", "3", "4", "5"]);
  });

  it("also finds names whose letters come in the typed order, after the plain matches (like fzf)", () => {
    const games = [
      { id: "a", name: "Slay the Spire", labels: [] },
      { id: "b", name: "Street Fighter 6", labels: [] },
      { id: "c", name: "催眠アプリで彼女に完全堕ち", labels: [] },
      { id: "d", name: "Spire", labels: [] },
    ];
    // "sts" and "stf" are in no name, but their letters come in order in one each.
    expect(ids(visibleItems(games, { query: "sts" }))).toEqual(["a"]);
    expect(ids(visibleItems(games, { query: "stf" }))).toEqual(["b"]);
    // A plain match comes before the scattered ones, each keeping the list's order.
    expect(ids(visibleItems(games, { query: "spire" }))).toEqual(["a", "d"]);
    expect(ids(visibleItems(games, { query: "sre" }))).toEqual(["a", "b", "d"]);
    expect(ids(visibleItems(games, { query: "催アプ" }))).toEqual(["c"]);
    expect(ids(visibleItems(games, { query: "zz" }))).toEqual([]);
  });

  it("matches the detail (the maker) as well as the name", () => {
    const games = [
      { id: "a", name: "夏のマゾ", detail: "3Djp_Art", labels: [] },
      { id: "b", name: "Other", detail: "Somebody", labels: [] },
    ];
    expect(ids(visibleItems(games, { query: "3djp" }))).toEqual(["a"]);
    expect(ids(visibleItems(games, { query: "3djp 夏" }))).toEqual(["a"]);
    expect(ids(visibleItems(games, { query: "3djp other" }))).toEqual([]);
  });

  it("matches every word of the query in the name, ignoring case and width", () => {
    expect(ids(visibleItems(items, { query: "spire" }))).toEqual(["2", "3"]);
    expect(ids(visibleItems(items, { query: "spire 2" }))).toEqual(["3"]);
    expect(ids(visibleItems(items, { query: "  SLAY   spire " }))).toEqual(["2", "3"]);
    expect(ids(visibleItems(items, { query: "remnant" }))).toEqual(["5"]);
    expect(ids(visibleItems(items, { query: "ｓｔｒｅｅｔ" }))).toEqual(["1"]);
    expect(visibleItems(items, { query: "zelda" })).toEqual([]);
  });

  it("keeps games in any of the selected labels", () => {
    expect(ids(visibleItems(items, { active: ["お気に入り", "R15"] }))).toEqual(["2", "3", "5"]);
    expect(ids(visibleItems(items, { active: ["R15"], query: "spire" }))).toEqual(["3"]);
  });

  it("「ラベル非登録」 keeps the games without any label, alone or with other labels", () => {
    expect(ids(visibleItems(items, { active: [UNLABELED], hide: ["outdate"] }))).toEqual(["1"]);
    expect(ids(visibleItems(items, { active: [UNLABELED, "お気に入り"] }))).toEqual(["1", "2"]);
    expect(ids(visibleItems(items, { active: [UNLABELED], query: "slay" }))).toEqual([]);
  });

  it("shows a hidden label's games only while that label is selected", () => {
    const hide = ["outdate"];
    expect(ids(visibleItems(items, { active: ["R15"], hide }))).toEqual(["3"]);
    expect(ids(visibleItems(items, { active: ["outdate"], hide }))).toEqual(["4", "5"]);
    expect(ids(visibleItems(items, { active: ["R15", "outdate"], hide }))).toEqual(["3", "4", "5"]);
    expect(ids(visibleItems(items, { query: "metal", hide }))).toEqual([]);
  });
});

describe("partialReason", () => {
  it("says in Japanese why only installed games are listed", () => {
    expect(partialReason(null)).toBeNull();
    expect(partialReason("secrets.yaml has no steam api_key")).toContain("API キー");
    expect(partialReason("the Steam Web API rejected the key")).toContain("受け付けません");
    expect(partialReason("the owned games are not fetched yet")).toContain("取得中");
    expect(partialReason("the Steam Web API cannot be reached: timeout")).toContain(
      "the Steam Web API cannot be reached: timeout",
    );
    // DLsite's folder.
    expect(partialReason("D:\\DLsiteNest\\Game does not exist")).toBe(
      "ゲームのフォルダ D:\\DLsiteNest\\Game がありません",
    );
    expect(partialReason("there are no games in D:\\Games")).toBe("D:\\Games にゲームがありません");
  });
});

describe("labelsLockedReason", () => {
  it("says in Japanese why labels cannot be changed", () => {
    expect(labelsLockedReason(null)).toBeNull();
    expect(labelsLockedReason("Steam is not running")).toContain("Steam が起動していない");
    expect(
      labelsLockedReason(
        "Steam does not accept remote control: create .cef-enable-remote-debugging in the Steam folder and restart Steam",
      ),
    ).toContain(".cef-enable-remote-debugging を置いて");
    expect(labelsLockedReason("timeout")).toBe("ラベルを変更できません: timeout");
  });
});

describe("pictureUrl", () => {
  it("asks windows-link for a game's picture", () => {
    expect(pictureUrl("http://127.0.0.1:4730", "games", "1364780")).toBe(
      "http://127.0.0.1:4730/buttons/games/library/1364780/image",
    );
    expect(pictureUrl("http://h", "a b", "x/y")).toBe("http://h/buttons/a%20b/library/x%2Fy/image");
  });
});

describe("pinsOf", () => {
  it("lists the pins of the library buttons in button order", () => {
    const pin = (id) => ({ id, name: id });
    const buttons = [
      { id: "games", state: { kind: "library", pins: [pin("1"), pin("2")], pictures: "cover" } },
      { id: "output", state: { kind: "output" } },
      { id: "more", state: { kind: "library", pins: [pin("3")], pictures: "icon" } },
    ];
    expect(pinsOf(buttons)).toEqual([
      { button: "games", pin: pin("1"), icon: false },
      { button: "games", pin: pin("2"), icon: false },
      { button: "more", pin: pin("3"), icon: true },
    ]);
  });
});
