<script>
  import { onMount } from "svelte";

  /** @type {{ x: number, y: number, title: string, items: Array<{ label: string, disabled?: boolean, onselect: () => void }>, onclose: () => void }} */
  let { x, y, title, items, onclose } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();
  let left = $state(0);
  let top = $state(0);

  onMount(() => {
    left = x;
    top = y;
    dialog?.showModal();
    // Open where the finger was, kept inside the window.
    const box = dialog?.getBoundingClientRect();
    if (box) {
      left = Math.max(8, Math.min(x, innerWidth - box.width - 8));
      top = Math.max(8, Math.min(y, innerHeight - box.height - 8));
    }
  });
</script>

<dialog
  bind:this={dialog}
  class="context"
  aria-label={title}
  style:left="{left}px"
  style:top="{top}px"
  oncancel={(event) => {
    event.preventDefault();
    onclose();
  }}
  onclick={(event) => {
    if (event.target === dialog) onclose();
  }}
>
  <div class="list" role="menu" aria-label={title}>
    <p class="title">{title}</p>
    {#each items as item (item.label)}
      <button
        type="button"
        role="menuitem"
        class="item"
        disabled={item.disabled}
        onclick={() => {
          // The choice runs first, so that it can keep its owner open for a next step.
          item.onselect();
          onclose();
        }}
      >
        {item.label}
      </button>
    {/each}
  </div>
</dialog>

<style lang="sass">
  .context
    position: fixed
    margin: 0
    padding: 0
    width: 320px
    max-width: calc(100vw - 16px)
    border: 1px solid var(--c-border)
    border-radius: 12px
    background: var(--c-surface-raised)
    color: var(--c-on-surface)
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.25)

    &::backdrop
      background: transparent

  .list
    display: flex
    flex-direction: column
    padding: var(--sp-2) 0

  .title
    margin: 0
    padding: var(--sp-2) var(--sp-4)
    color: var(--c-muted)
    font-size: var(--fs-sm)
    overflow: hidden
    text-overflow: ellipsis
    white-space: nowrap

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
    touch-action: manipulation
    -webkit-tap-highlight-color: transparent

    &:disabled
      opacity: 0.5
      cursor: default

  @media (hover: hover)
    .item:not(:disabled):hover
      background: var(--c-hover-1)
</style>
