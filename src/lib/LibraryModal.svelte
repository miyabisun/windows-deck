<script>
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";
  import Tile from "./Tile.svelte";
  import { partialReason, pictureUrl, visibleItems } from "./library.js";
  import { itemKey } from "./link.svelte.js";

  const NOTICE_MS = 3000;

  /** @type {{ link: import("./link.svelte.js").Link, button: any, onclose: () => void }} */
  let { link, button, onclose } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();
  /** @type {{ items: any[], labels: string[], hide: string[], partial: string | null } | null} */
  let library = $state(null);
  /** @type {string | null} */
  let failure = $state(null);
  let query = $state("");
  /** @type {string[]} */
  let active = $state([]);
  /** @type {string | null} what the last long press did, shown for a few seconds */
  let notice = $state(null);
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let noticeTimer;

  const shown = $derived(
    library ? visibleItems(library.items, { query, active, hide: library.hide }) : [],
  );
  // From the button's live state, so a pin made here shows at once.
  const pinned = $derived(new Set((button.state?.pins ?? []).map((/** @type {any} */ p) => p.id)));
  const disabled = $derived(link.status !== "connected");

  onMount(() => {
    // A modal dialog keeps the focus inside and starts it on the search field.
    dialog?.showModal();
    link.library(button.id).then((read) => {
      library = read.library;
      failure = read.failure;
    });
    return () => clearTimeout(noticeTimer);
  });

  /** @param {string} label */
  function toggle(label) {
    active = active.includes(label) ? active.filter((l) => l !== label) : [...active, label];
  }

  /** @param {any} item */
  async function start(item) {
    if (await link.startItem(button.id, item.id)) onclose();
  }

  /** @param {any} item */
  async function togglePin(item) {
    const pin = !pinned.has(item.id);
    const failed = await link.setPinned(button.id, item.id, pin);
    // A failure stays on the game's tile instead.
    if (failed) return;
    notice = `「${item.name}」を TOP ${pin ? "に固定しました" : "から外しました"}`;
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = null), NOTICE_MS);
  }

  /** @param {any} item */
  function noteOf(item) {
    const notes = [];
    if (pinned.has(item.id)) notes.push("TOP に固定中");
    if (!item.installed) notes.push("未インストール（押すとインストール）");
    return notes.join("・") || null;
  }
</script>

<dialog
  bind:this={dialog}
  class="library"
  aria-label={button.label}
  oncancel={(event) => {
    event.preventDefault();
    onclose();
  }}
  onclick={(event) => {
    // Only the backdrop is the dialog itself; the panel fills the rest.
    if (event.target === dialog) onclose();
  }}
>
  <div class="panel">
    <div class="head">
      <input
        type="search"
        class="search"
        placeholder="名前で探す"
        aria-label="名前で探す"
        bind:value={query}
      />
      <button type="button" class="close" aria-label="閉じる" onclick={onclose}>
        <Icon name="close" />
      </button>
    </div>
    {#if library?.labels.length}
      <div class="chips" role="group" aria-label="ラベルで絞り込む">
        {#each library.labels as label (label)}
          <button
            type="button"
            class="chip"
            aria-pressed={active.includes(label)}
            onclick={() => toggle(label)}
          >
            {#if active.includes(label)}
              <Icon name="check" />
            {/if}
            {label}
          </button>
        {/each}
      </div>
    {/if}
    {#if partialReason(library?.partial ?? null)}
      <p class="partial">{partialReason(library?.partial ?? null)}</p>
    {/if}
    <p class="notice" role="status">{notice ?? ""}</p>
    <div class="scroll">
      <div class="items">
        {#if failure}
          <p class="failure" role="alert">{failure}</p>
        {:else if !library}
          <p class="quiet" role="status"><Spinner /> 読み込んでいます…</p>
        {:else}
          {#each shown as item (item.id)}
            <Tile
              button={{ id: item.id, label: item.name }}
              cover={pictureUrl(link.base, button.id, item.id)}
              note={noteOf(item)}
              pending={!!link.pending[itemKey(button.id, item.id)]}
              failure={link.failures[itemKey(button.id, item.id)]}
              {disabled}
              onpress={() => start(item)}
              onlong={() => togglePin(item)}
            />
          {:else}
            <p class="quiet">見つかりません</p>
          {/each}
        {/if}
      </div>
    </div>
  </div>
</dialog>

<style lang="sass">
  .library
    width: 80vw
    height: 80vh
    max-width: none
    max-height: none
    box-sizing: border-box
    padding: 0
    border: 1px solid var(--c-border)
    border-radius: 12px
    background: var(--c-surface)
    color: var(--c-on-surface)
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.25)

    &::backdrop
      background: rgba(0, 0, 0, 0.5)

  .panel
    position: relative
    display: flex
    flex-direction: column
    gap: var(--sp-3)
    height: 100%
    box-sizing: border-box
    padding: var(--sp-4)

  .head
    display: flex
    gap: var(--sp-3)

  .search
    flex: 1
    min-width: 0
    height: 56px
    padding: 0 var(--sp-4)
    border: 1px solid var(--c-border)
    border-radius: var(--radius-md)
    background: var(--c-surface-raised)
    color: inherit
    font: inherit
    font-size: var(--fs-label)
    // The field takes text selection back from the page-wide "no selection".
    -webkit-user-select: text
    user-select: text

  .close
    display: inline-flex
    align-items: center
    justify-content: center
    flex: none
    width: 56px
    height: 56px
    border: 0
    border-radius: var(--radius-md)
    background: transparent
    color: var(--c-on-surface)
    font-size: var(--fs-state)
    cursor: pointer
    touch-action: manipulation
    -webkit-tap-highlight-color: transparent

  .chips
    display: flex
    flex-wrap: wrap
    gap: var(--sp-2)

  // Big enough for a finger: as tall as the search field.
  .chip
    display: inline-flex
    align-items: center
    gap: var(--sp-2)
    min-height: 56px
    padding: 0 var(--sp-5)
    border: 1px solid var(--c-border)
    border-radius: 28px
    background: var(--c-surface-raised)
    color: var(--c-on-surface)
    font: inherit
    font-size: var(--fs-label)
    cursor: pointer
    touch-action: manipulation
    -webkit-tap-highlight-color: transparent

    &[aria-pressed="true"]
      border-color: var(--c-accent)
      background: var(--c-accent-subtle)
      font-weight: 600

  @media (hover: hover)
    .close:hover,
    .chip[aria-pressed="false"]:hover
      background: var(--c-hover-1)

  .partial
    margin: 0
    color: var(--c-muted)
    font-size: var(--fs-sm)
    line-height: 1.5

  // A toast over the bottom of the list; always present so that it is announced.
  .notice
    position: absolute
    bottom: var(--sp-5)
    left: 50%
    max-width: calc(100% - 2 * var(--sp-5))
    margin: 0
    padding: var(--sp-3) var(--sp-4)
    border: 1px solid var(--c-border)
    border-radius: var(--radius-md)
    background: var(--c-surface-raised)
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.25)
    font-size: var(--fs-md)
    transform: translateX(-50%)
    pointer-events: none

    &:empty
      opacity: 0

  // The list scrolls inside the dialog; the search and labels stay in place. The grid
  // itself has no fixed height, so that its rows fit the tiles.
  .scroll
    flex: 1
    min-height: 0
    overflow-y: auto

  .items
    display: grid
    grid-template-columns: repeat(auto-fill, minmax(var(--tile-width), 1fr))
    align-content: start
    gap: var(--sp-3)

  .quiet,
  .failure
    display: flex
    align-items: center
    gap: var(--sp-2)
    margin: 0
    grid-column: 1 / -1

  .quiet
    color: var(--c-muted)

  .failure
    color: var(--c-danger)
</style>
