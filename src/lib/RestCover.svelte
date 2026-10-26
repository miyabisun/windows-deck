<script>
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";

  /** @type {{ pending?: boolean, failure?: string | null, onwake: () => void }} */
  let { pending = false, failure = null, onwake } = $props();
  /** @type {HTMLButtonElement | undefined} */
  let wake = $state();

  // The only thing to do while resting, so it has the focus.
  $effect(() => wake?.focus());
</script>

<div class="rest" role="dialog" aria-modal="true" aria-label="休止中">
  <button
    type="button"
    class="wake"
    aria-busy={pending}
    bind:this={wake}
    onclick={() => {
      if (!pending) onwake();
    }}
  >
    {#if pending}<Spinner />{:else}<Icon name="sun" />{/if}
    復帰
  </button>
  {#if failure}
    <p class="failure" role="status">{failure}</p>
  {/if}
</div>

<style lang="sass">
  // Over the whole panel, which shows through dim and grey and cannot be pressed. What is
  // on it is drawn for that dark ground in either theme: faint white on black, never a
  // bright face, and nothing under it shows through the button.
  .rest
    position: fixed
    inset: 0
    z-index: 2
    display: flex
    flex-direction: column
    align-items: center
    justify-content: center
    gap: var(--sp-4)
    padding: var(--sp-5)
    backdrop-filter: grayscale(1) brightness(0.3)

  .wake
    display: inline-flex
    align-items: center
    justify-content: center
    gap: var(--sp-3)
    min-width: var(--tile-width)
    min-height: var(--tile-height)
    padding: 0 var(--sp-5)
    border: 1px solid rgb(255 255 255 / 0.3)
    border-radius: var(--radius-md)
    background: rgb(0 0 0 / 0.6)
    color: rgb(255 255 255 / 0.8)
    font: inherit
    font-size: var(--fs-state)
    font-weight: 600
    cursor: pointer
    touch-action: manipulation

  .failure
    max-width: 40em
    margin: 0
    padding: var(--sp-3)
    border: 1px solid rgb(255 255 255 / 0.3)
    border-radius: var(--radius-sm)
    color: rgb(255 255 255 / 0.8)
    font-size: var(--fs-sm)
    line-height: 1.5
</style>
