<script>
  import { onMount } from "svelte";
  import ContextMenu from "./lib/ContextMenu.svelte";
  import DesktopTabs from "./lib/DesktopTabs.svelte";
  import GameMenu from "./lib/GameMenu.svelte";
  import LibraryModal from "./lib/LibraryModal.svelte";
  import MixerModal from "./lib/MixerModal.svelte";
  import ProgramChooser from "./lib/ProgramChooser.svelte";
  import Spinner from "./lib/Spinner.svelte";
  import Tile from "./lib/Tile.svelte";
  import Toast, { Notice } from "./lib/Toast.svelte";
  import { stateIcon } from "./lib/display.js";
  import { pictureUrl, pinsOf } from "./lib/library.js";
  import { Link, itemKey } from "./lib/link.svelte.js";
  import { buttonsFor } from "./lib/tabs.js";
  import { loadConfig, placeWindow, windowHandle } from "./lib/window.js";

  /** @type {Link | null} */
  let link = $state(null);
  let configError = $state(null);
  /** Whether the window has been placed and shown, so it can be pinned. */
  let shown = $state(false);
  /** @type {number | null} */
  let hwnd = null;
  /** @type {{ path: string, candidates: import("./lib/placement.js").Monitor[] } | null} no monitor is configured */
  let setup = $state(null);

  const visible = $derived(link ? buttonsFor(link.buttons, link.desktops) : []);
  const pins = $derived(pinsOf(visible));
  /** @type {string | null} the library button whose list is open */
  let opened = $state(null);
  /** whether the mixer is open */
  let mixing = $state(false);
  const openedButton = $derived(link?.buttons.find((b) => b.id === opened) ?? null);
  /** @type {{ button: string, pin: { id: string, name: string }, x: number, y: number } | null} the pinned game whose menu is open */
  let pinMenu = $state(null);
  /** @type {{ button: string, pin: { id: string, name: string } } | null} the pinned game whose program is being chosen */
  let pinChoice = $state(null);
  /** @type {{ button: string, label: string, x: number, y: number } | null} the library button whose menu is open */
  let libraryMenu = $state(null);
  /** what the last library update started, shown for a few seconds */
  const notice = new Notice();

  /** @param {string} button */
  async function updateLibrary(button) {
    const started = await link?.updateLibrary(button);
    // A failure stays on the button (Link keeps it).
    if (started) notice.show(started);
  }

  /** @param {string} button @param {{ id: string, name: string }} pin */
  async function startPin(button, pin) {
    if ((await link?.startItem(button, pin.id)) === "choose") pinChoice = { button, pin };
  }

  onMount(() => {
    // A long press would open the WebView's context menu.
    const noMenu = (/** @type {Event} */ event) => event.preventDefault();
    document.addEventListener("contextmenu", noMenu);
    let started = null;
    loadConfig().then(async (config) => {
      configError = config.error;
      started = new Link(config.link);
      link = started;
      started.start();
      hwnd = await windowHandle();
      const placed = await placeWindow(config.link, config.monitor);
      if (placed?.kind === "setup") setup = { path: config.path, candidates: placed.candidates };
      shown = true;
    });
    return () => {
      document.removeEventListener("contextmenu", noMenu);
      started?.stop();
      notice.stop();
    };
  });

  // Stay on every virtual desktop; see Link.pinRound for when to pin again.
  $effect(() => {
    if (link?.pinRound && shown && hwnd !== null) link.pin(hwnd);
  });
</script>

{#if link}
  <DesktopTabs
    desktops={link.desktops}
    unmatched={link.unmatched}
    switching={link.switching}
    failure={link.desktopFailure}
    disabled={link.status !== "connected"}
    onswitch={(id) => link.switchDesktop(id)}
    oncreate={(name) => link.createDesktop(name)}
    onsleep={() => link.sleep()}
  />
{/if}

<main class="deck">
  {#if configError}
    <p class="banner" role="alert">
      設定ファイルを読めないため、既定の設定で動いています: {configError}
    </p>
  {/if}
  {#if setup}
    <div class="banner setup" role="alert">
      <p>
        表示するモニターが設定されていません。{setup.path || "設定ファイル"} に
        <code>monitor: モニターのID</code> を書いて、再読み込みしてください。
      </p>
      {#if setup.candidates.length > 0}
        <ul>
          {#each setup.candidates as candidate (candidate.id)}
            <li>
              <code>{candidate.id}</code>
              {candidate.name.trim()}{candidate.primary ? "（メイン）" : ""}{candidate.touch
                ? "（タッチ）"
                : ""}
            </li>
          {/each}
        </ul>
      {/if}
      <button type="button" class="reload" onclick={() => location.reload()}>再読み込み</button>
    </div>
  {/if}
  {#if link?.status === "disconnected"}
    <p class="banner" role="status">
      windows-link（{link.base}）に接続できません。再接続しています…
    </p>
  {/if}

  {#if !link || (link.status === "connecting" && link.buttons.length === 0)}
    <p class="quiet" role="status"><Spinner /> windows-link に接続しています…</p>
  {:else if link.status === "connected" && link.buttons.length === 0}
    <div class="quiet">
      <p>ボタンがありません。</p>
      <p>
        windows-link
        の設定ファイル（%LOCALAPPDATA%\windows-link\config.yaml）にボタンを追加し、windows-link
        を再起動してください。
      </p>
    </div>
  {:else if visible.length === 0}
    <p class="quiet">このデスクトップに割り当てたボタンはありません。</p>
  {:else}
    <div class="grid">
      {#each visible as button (button.id)}
        <Tile
          {button}
          icon={button.icon ? `${link.base}/buttons/${encodeURIComponent(button.id)}/icon` : null}
          glyph={stateIcon(button.state)}
          pending={!!link.pending[button.id]}
          failure={link.failures[button.id]}
          disabled={link.status !== "connected"}
          onpress={() => {
            if (button.state?.kind === "library") {
              // Opening the library is the next press, which clears a failed update.
              delete link.failures[button.id];
              opened = button.id;
            } else if (button.state?.kind === "mixer") mixing = true;
            else link.press(button.id);
          }}
          onlong={button.state?.kind === "library"
            ? (point) => (libraryMenu = { button: button.id, label: button.label, ...point })
            : null}
        />
      {/each}
      {#each pins as { button, pin, whole } (itemKey(button, pin.id))}
        <Tile
          button={{ id: itemKey(button, pin.id), label: pin.name }}
          cover={pictureUrl(link.base, button, pin.id)}
          {whole}
          pending={!!link.pending[itemKey(button, pin.id)]}
          failure={link.failures[itemKey(button, pin.id)]}
          disabled={link.status !== "connected"}
          onpress={() => startPin(button, pin)}
          onlong={(point) => (pinMenu = { button, pin, ...point })}
        />
      {/each}
    </div>
  {/if}
</main>

{#if link && openedButton}
  <LibraryModal {link} button={openedButton} onclose={() => (opened = null)} />
{/if}

{#if link && mixing}
  <MixerModal {link} onclose={() => (mixing = false)} />
{/if}

{#if link && pinChoice}
  <ProgramChooser
    {link}
    button={pinChoice.button}
    item={pinChoice.pin}
    start
    onclose={() => (pinChoice = null)}
  />
{/if}

{#if link && libraryMenu}
  <ContextMenu
    x={libraryMenu.x}
    y={libraryMenu.y}
    title={libraryMenu.label}
    items={[{ label: "ライブラリを更新", onselect: () => updateLibrary(libraryMenu.button) }]}
    onclose={() => (libraryMenu = null)}
  />
{/if}

<Toast {notice} fixed />

{#if link && pinMenu}
  <GameMenu
    {link}
    button={pinMenu.button}
    item={pinMenu.pin}
    pinned
    x={pinMenu.x}
    y={pinMenu.y}
    onclose={() => (pinMenu = null)}
  />
{/if}

<style lang="sass">
  .deck
    display: flex
    flex-direction: column
    gap: var(--sp-3)
    padding: var(--sp-4)

  .grid
    display: grid
    grid-template-columns: repeat(auto-fill, minmax(var(--tile-width), 1fr))
    gap: var(--sp-3)

  .banner
    margin: 0
    padding: var(--sp-3)
    border-radius: var(--radius-sm)
    background: var(--c-danger-subtle)
    color: var(--c-danger)
    font-size: var(--fs-sm)
    line-height: 1.5

  // No monitor is configured: the monitors to choose from and a way to read the file again.
  .setup
    display: flex
    flex-direction: column
    align-items: flex-start
    gap: var(--sp-2)

    p, ul
      margin: 0

    ul
      padding-left: var(--sp-5)

  .reload
    min-height: 56px
    padding: 0 var(--sp-5)
    border: 1px solid currentColor
    border-radius: var(--radius-md)
    background: transparent
    color: inherit
    font: inherit
    cursor: pointer
    touch-action: manipulation

  .quiet
    color: var(--c-muted)

    p
      margin: 0 0 var(--sp-2)

  p.quiet
    display: flex
    align-items: center
    gap: var(--sp-2)
    margin: 0
</style>
