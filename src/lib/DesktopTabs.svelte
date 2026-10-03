<script>
  import Spinner from "./Spinner.svelte";

  /** @type {{ desktops: Array<{ id: string, name: string, index: number, current: boolean }>, switching?: string | null, failure?: string | null, disabled?: boolean, onswitch: (id: string) => void }} */
  let { desktops, switching = null, failure = null, disabled = false, onswitch } = $props();

  const ordered = $derived([...desktops].sort((a, b) => a.index - b.index));
  // Only the current tab is in the Tab order; the arrow keys move between tabs and
  // Enter or Space switches (tabs pattern with manual activation).
  const focusable = $derived(ordered.some((d) => d.current) ? null : ordered[0]?.id);

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
</script>

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
    gap: var(--sp-1)
    min-width: 0
    overflow-x: auto

  .tab
    display: inline-flex
    align-items: center
    gap: var(--sp-2)
    flex: none
    height: 48px
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

  @media (hover: hover)
    .tab[aria-selected="false"]:hover
      background: var(--c-hover-1)

  .failure
    margin: 0
    color: var(--c-danger)
    font-size: var(--fs-sm)
</style>
