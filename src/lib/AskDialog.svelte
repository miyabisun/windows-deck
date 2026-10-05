<script>
  import { onMount } from "svelte";

  /** @type {{ title: string, message?: string | null, value?: string | null, confirm: string, danger?: boolean, onconfirm: (value: string) => void, oncancel: () => void }} */
  let {
    title,
    message = null,
    value = null,
    confirm,
    danger = false,
    onconfirm,
    oncancel,
  } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();
  // With `value`, the dialog asks for a name and starts with it.
  let text = $state("");
  const asking = $derived(value !== null);
  const ready = $derived(!asking || text.trim() !== "");

  onMount(() => {
    text = value ?? "";
    dialog?.showModal();
  });
</script>

<dialog
  bind:this={dialog}
  class="ask"
  aria-label={title}
  oncancel={(event) => {
    event.preventDefault();
    oncancel();
  }}
>
  <form
    class="body"
    onsubmit={(event) => {
      event.preventDefault();
      if (ready) onconfirm(text.trim());
    }}
  >
    <h2 class="title">{title}</h2>
    {#if message}
      <p class="message">{message}</p>
    {/if}
    {#if asking}
      <input class="name" type="text" aria-label={title} bind:value={text} />
    {/if}
    <div class="buttons">
      <button type="button" class="button" onclick={oncancel}>やめる</button>
      <button type="submit" class="button primary" class:danger disabled={!ready}>{confirm}</button>
    </div>
  </form>
</dialog>

<style lang="sass">
  .ask
    width: 480px
    max-width: calc(100vw - 32px)
    padding: 0
    border: 1px solid var(--c-border)
    border-radius: 12px
    background: var(--c-surface-raised)
    color: var(--c-on-surface)
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.25)

    &::backdrop
      background: rgba(0, 0, 0, 0.5)

  .body
    display: flex
    flex-direction: column
    gap: var(--sp-4)
    padding: var(--sp-5)

  .title
    margin: 0
    font-size: var(--fs-label)
    font-weight: 600

  .message
    margin: 0
    color: var(--c-muted)

  .name
    height: 56px
    padding: 0 var(--sp-4)
    border: 1px solid var(--c-border)
    border-radius: var(--radius-md)
    background: var(--c-surface)
    color: inherit
    font: inherit
    font-size: var(--fs-label)
    -webkit-user-select: text
    user-select: text

  .buttons
    display: flex
    justify-content: flex-end
    gap: var(--sp-3)

  .button
    min-width: 120px
    min-height: 56px
    padding: 0 var(--sp-5)
    border: 1px solid var(--c-border)
    border-radius: var(--radius-md)
    background: transparent
    color: var(--c-on-surface)
    font: inherit
    font-size: var(--fs-label)
    cursor: pointer
    touch-action: manipulation

    &:disabled
      opacity: 0.5
      cursor: default

  .primary
    border-color: var(--c-accent)
    font-weight: 600

  .danger
    border-color: var(--c-danger)
    color: var(--c-danger)

  @media (hover: hover)
    .button:not(:disabled):hover
      background: var(--c-hover-1)
</style>
