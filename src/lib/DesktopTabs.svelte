<script>
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";

  /** @type {{ desktops: Array<{ id: string, name: string, index: number, current: boolean }>, unmatched?: string[], switching?: string | null, failure?: string | null, disabled?: boolean, onswitch: (id: string) => void, oncreate: (name: string) => void, onsleep: () => void }} */
  let {
    desktops,
    unmatched = [],
    switching = null,
    failure = null,
    disabled = false,
    onswitch,
    oncreate,
    onsleep,
  } = $props();

  const ordered = $derived([...desktops].sort((a, b) => a.index - b.index));
  // Only the current tab is in the Tab order; the arrow keys move between tabs and
  // Enter or Space switches (tabs pattern with manual activation).
  const focusable = $derived(ordered.some((d) => d.current) ? null : ordered[0]?.id);
  let menuOpen = $state(false);
  /** @type {HTMLButtonElement | undefined} */
  let addButton = $state();

  /** @param {KeyboardEvent} event */
  function move(event) {
    const step = { ArrowRight: 1, ArrowLeft: -1 }[event.key];
    const list = /** @type {HTMLElement} */ (event.currentTarget);
    const all = [...list.querySelectorAll("[role=tab]")];
    let next;
    if (step) next = all[(all.indexOf(document.activeElement) + step + all.length) % all.length];
    else if (event.key === "Home") next = all[0];
    else if (event.key === "End") next = all.at(-1);
    if (!next) return;
    event.preventDefault();
    /** @type {HTMLElement} */ (next).focus();
  }

  function closeMenu() {
    menuOpen = false;
    addButton?.focus();
  }
</script>

<svelte:window
  onkeydown={(event) => {
    if (menuOpen && event.key === "Escape") closeMenu();
  }}
/>

<nav class="tabs" aria-label="仮想デスクトップ">
  <div class="list" role="tablist" tabindex="-1" onkeydown={move}>
    {#each ordered as desktop (desktop.id)}
      <button
        type="button"
        role="tab"
        class="tab"
        aria-selected={desktop.current}
        tabindex={desktop.current || desktop.id === focusable ? 0 : -1}
        aria-busy={switching === desktop.id}
        aria-disabled={disabled}
        data-desktop={desktop.id}
        onclick={() => {
          if (!disabled && !desktop.current) onswitch(desktop.id);
        }}
      >
        {desktop.name}
        {#if switching === desktop.id}
          <Spinner />
        {/if}
      </button>
    {/each}
  </div>
  {#if failure}
    <p class="failure" role="alert">{failure}</p>
  {/if}
  <div class="actions">
    <button
      type="button"
      class="action"
      aria-label="デスクトップを作る"
      aria-haspopup="menu"
      aria-expanded={menuOpen}
      aria-disabled={disabled}
      bind:this={addButton}
      onclick={() => {
        if (!disabled) menuOpen = !menuOpen;
      }}
    >
      <Icon name="plus" />
    </button>
    <button
      type="button"
      class="action"
      aria-label="スリープ"
      aria-disabled={disabled}
      onclick={() => {
        if (!disabled) onsleep();
      }}
    >
      <Icon name="moon" />
    </button>
  </div>
  {#if menuOpen}
    <button type="button" class="backdrop" aria-label="閉じる" tabindex="-1" onclick={closeMenu}
    ></button>
    <div class="menu" role="menu" aria-label="作れるデスクトップ">
      {#each unmatched as name (name)}
        <button
          type="button"
          role="menuitem"
          class="item"
          onclick={() => {
            menuOpen = false;
            oncreate(name);
          }}
        >
          {name}
        </button>
      {:else}
        <p class="empty">
          作れるデスクトップはありません。windows-link の desktops
          フォルダに「名前.yaml」を置くと、ここに出ます。
        </p>
      {/each}
    </div>
  {/if}
</nav>

<style lang="sass">
  .tabs
    position: sticky
    top: 0
    z-index: 1
    display: flex
    align-items: center
    gap: var(--sp-3)
    padding: 0 var(--sp-4)
    border-bottom: 1px solid var(--c-border)
    background: var(--c-surface)

  .list
    display: flex
    flex: 1
    gap: var(--sp-1)
    min-width: 0
    overflow-x: auto

  // A tab is half a button: half its height, and at least half its width.
  .tab
    display: inline-flex
    align-items: center
    justify-content: center
    gap: var(--sp-2)
    flex: none
    height: calc(var(--tile-height) / 2)
    min-width: calc(var(--tile-width) / 2)
    padding: 0 var(--sp-5)
    border: 0
    background: transparent
    color: var(--c-muted)
    font: inherit
    font-size: var(--fs-label)
    white-space: nowrap
    cursor: pointer
    touch-action: manipulation
    -webkit-tap-highlight-color: transparent

    &[aria-selected="true"]
      color: var(--c-on-surface)
      font-weight: 600
      box-shadow: inset 0 -3px 0 var(--c-accent)
      cursor: default

    &[aria-disabled="true"]
      opacity: 0.5
      pointer-events: none

  .actions
    display: flex
    flex: none
    gap: var(--sp-1)

  .action
    display: inline-flex
    align-items: center
    justify-content: center
    width: calc(var(--tile-height) / 2)
    height: calc(var(--tile-height) / 2)
    border: 0
    background: transparent
    color: var(--c-on-surface)
    font-size: var(--fs-state)
    cursor: pointer
    touch-action: manipulation
    -webkit-tap-highlight-color: transparent

    &[aria-disabled="true"]
      opacity: 0.5
      pointer-events: none

  @media (hover: hover)
    .tab[aria-selected="false"]:hover,
    .action:hover
      background: var(--c-hover-1)

  .failure
    margin: 0
    color: var(--c-danger)
    font-size: var(--fs-sm)

  .backdrop
    position: fixed
    inset: 0
    border: 0
    background: transparent

  .menu
    position: absolute
    top: 100%
    right: var(--sp-4)
    display: flex
    flex-direction: column
    min-width: 240px
    max-width: min(480px, calc(100vw - 32px))
    padding: var(--sp-2) 0
    border: 1px solid var(--c-border)
    border-radius: 12px
    background: var(--c-surface-raised)
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.25)

  .item
    min-height: var(--row-height)
    padding: 0 var(--sp-4)
    border: 0
    background: transparent
    color: var(--c-on-surface)
    font: inherit
    font-size: var(--fs-label)
    text-align: left
    cursor: pointer

  @media (hover: hover)
    .item:hover
      background: var(--c-hover-1)

  .empty
    margin: 0
    padding: var(--sp-3) var(--sp-4)
    color: var(--c-muted)
    font-size: var(--fs-sm)
</style>
