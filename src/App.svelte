<script>
  import { onMount } from "svelte";
  import Spinner from "./lib/Spinner.svelte";
  import Tile from "./lib/Tile.svelte";
  import { Link } from "./lib/link.svelte.js";
  import { loadConfig, placeWindow } from "./lib/window.js";

  /** @type {Link | null} */
  let link = $state(null);
  let configError = $state(null);
  let monitor = null;
  let placed = false;

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
      placed = await placeWindow(config.link, monitor);
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
</script>

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
  {:else}
    <div class="grid">
      {#each link.buttons as button (button.id)}
        <Tile
          {button}
          pending={!!link.pending[button.id]}
          failure={link.failures[button.id]}
          disabled={link.status !== "connected"}
          onpress={() => link.press(button.id)}
        />
      {/each}
    </div>
  {/if}
</main>

<style lang="sass">
  .deck
    display: flex
    flex-direction: column
    gap: var(--sp-3)
    padding: var(--sp-4)

  .grid
    display: grid
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr))
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
