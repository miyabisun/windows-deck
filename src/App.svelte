<script>
  import { onMount } from "svelte";
  import DesktopTabs from "./lib/DesktopTabs.svelte";
  import LibraryModal from "./lib/LibraryModal.svelte";
  import Spinner from "./lib/Spinner.svelte";
  import Tile from "./lib/Tile.svelte";
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
  let monitor = null;
  let placed = false;

  const visible = $derived(link ? buttonsFor(link.buttons, link.desktops) : []);
  const pins = $derived(pinsOf(visible));
  /** @type {string | null} the library button whose list is open */
  let opened = $state(null);
  const openedButton = $derived(link?.buttons.find((b) => b.id === opened) ?? null);

  onMount(() => {
    // A long press would open the WebView's context menu.
    const noMenu = (/** @type {Event} */ event) => event.preventDefault();
    document.addEventListener("contextmenu", noMenu);
    let started = null;
    loadConfig().then(async (config) => {
      configError = config.error;
      monitor = config.monitor;
      started = new Link(config.link);
      link = started;
      started.start();
      hwnd = await windowHandle();
      placed = await placeWindow(config.link, monitor);
      shown = true;
    });
    return () => {
      document.removeEventListener("contextmenu", noMenu);
      started?.stop();
    };
  });

  // Started before windows-link answered: move to the right monitor once it does.
  $effect(() => {
    if (link?.status === "connected" && !placed) {
      placed = true;
      placeWindow(link.base, monitor);
    }
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
          pending={!!link.pending[button.id]}
          failure={link.failures[button.id]}
          disabled={link.status !== "connected"}
          onpress={() =>
            button.state?.kind === "library" ? (opened = button.id) : link.press(button.id)}
        />
      {/each}
      {#each pins as { button, pin } (itemKey(button, pin.id))}
        <Tile
          button={{ id: itemKey(button, pin.id), label: pin.name }}
          cover={pictureUrl(link.base, button, pin.id)}
          pending={!!link.pending[itemKey(button, pin.id)]}
          failure={link.failures[itemKey(button, pin.id)]}
          disabled={link.status !== "connected"}
          onpress={() => link.startItem(button, pin.id)}
          onlong={() => link.setPinned(button, pin.id, false)}
        />
      {/each}
    </div>
  {/if}
</main>

{#if link && openedButton}
  <LibraryModal {link} button={openedButton} onclose={() => (opened = null)} />
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
