<script>
  import Spinner from "./Spinner.svelte";
  import { describeState } from "./display.js";
  import { longpress } from "./longpress.js";

  /** @type {{ button: { id: string, label: string, state?: any }, icon?: string | null, cover?: string | null, note?: string | null, pending?: boolean, failure?: string, disabled?: boolean, onpress: () => void, onlong?: (() => void) | null }} */
  let {
    button,
    icon = null,
    cover = undefined,
    note: extra = null,
    pending = false,
    failure = undefined,
    disabled = false,
    onpress,
    onlong = null,
  } = $props();

  // A game tile (any `cover`, even null) shows the game's picture and name instead of a
  // button state.
  const game = $derived(cover !== undefined);
  const shown = $derived(
    game ? { text: null, note: extra, failed: false } : describeState(button.state),
  );
  const note = $derived(failure ?? shown.note);
  let broken = $state(false);
  // After a tap the browser keeps :hover and :active on the tile, so the highlight follows
  // the pointer events instead: hover only for a mouse, pressed only while held down.
  let pointer = $state("mouse");
  let held = $state(false);
</script>

<button
  type="button"
  class="tile"
  class:game
  class:mouse={pointer === "mouse"}
  class:held
  class:failed={failure || shown.failed}
  onpointerover={(event) => (pointer = event.pointerType)}
  onpointerdown={(event) => {
    pointer = event.pointerType;
    held = true;
  }}
  onpointerup={() => (held = false)}
  onpointercancel={() => (held = false)}
  onpointerleave={() => (held = false)}
  aria-busy={pending}
  aria-disabled={disabled}
  data-button={button.id}
  use:longpress={disabled ? null : onlong}
  onclick={() => {
    if (!disabled) onpress();
  }}
>
  {#if game}
    {#if cover && !broken}
      <img class="cover" src={cover} alt="" draggable="false" onerror={() => (broken = true)} />
    {/if}
    <span class="label">{button.label}</span>
  {:else}
    <span class="head">
      {#if icon}
        <img class="picture" src={icon} alt="" draggable="false" />
      {/if}
      <span class="label">{button.label}</span>
    </span>
    <span class="state">{shown.text}</span>
  {/if}
  {#if note}
    <span class="note" class:failure={failure || shown.failed}>{note}</span>
  {/if}
  {#if pending}
    <span class="busy"><Spinner /></span>
  {/if}
</button>

<style lang="sass">
  .tile
    position: relative
    display: flex
    flex-direction: column
    align-items: flex-start
    gap: var(--sp-2)
    min-height: var(--tile-height)
    padding: var(--sp-4)
    border: 1px solid var(--c-border)
    border-radius: var(--radius-md)
    background: var(--c-surface-raised)
    color: inherit
    font: inherit
    text-align: left
    cursor: pointer
    touch-action: manipulation
    -webkit-tap-highlight-color: transparent

    &.mouse:hover
      background: var(--c-hover-1)

    &.held
      background: var(--c-hover-2)

    &[aria-disabled="true"]
      opacity: 0.5
      cursor: default
      pointer-events: none

    &.failed
      border-color: var(--c-danger)

  // The picture runs to the edges; the name and note keep the usual inset.
  .game
    gap: 0
    padding: 0
    overflow: hidden

    .label
      padding: var(--sp-3) var(--sp-4) var(--sp-2)

    .note
      padding: 0 var(--sp-4) var(--sp-3)

    .busy
      padding: var(--sp-1)
      border-radius: 50%
      background: var(--c-surface-raised)

  .cover
    display: block
    width: 100%
    aspect-ratio: 460 / 215
    object-fit: cover

  .head
    display: flex
    align-items: center
    gap: var(--sp-3)
    min-width: 0

  .picture
    width: 56px
    height: 56px
    flex: none
    border-radius: var(--radius-sm)
    object-fit: contain

  .label
    font-size: var(--fs-label)
    font-weight: 600
    line-height: 1.3
    padding-right: var(--sp-5)
    display: -webkit-box
    -webkit-line-clamp: 2
    line-clamp: 2
    -webkit-box-orient: vertical
    overflow: hidden

  .state
    font-size: var(--fs-state)
    font-weight: 600
    line-height: 1.25
    display: -webkit-box
    -webkit-line-clamp: 2
    line-clamp: 2
    -webkit-box-orient: vertical
    overflow: hidden
    overflow-wrap: anywhere

  .note
    font-size: var(--fs-sm)
    line-height: 1.5
    color: var(--c-muted)

    &.failure
      color: var(--c-danger)

  .busy
    position: absolute
    top: var(--sp-4)
    right: var(--sp-4)
    display: flex
</style>
