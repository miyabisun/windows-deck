<script>
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";

  /** How long "コピーしました" stays. */
  const COPIED_MS = 3000;

  /** @type {{ link: import("./link.svelte.js").Link, button: string, item: { id: string, name: string }, onclose: () => void }} */
  let { link, button, item, onclose } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();
  /** @type {Array<{ label: string, value: string }> | null} */
  let keys = $state(null);
  /** @type {string | null} */
  let failure = $state(null);
  /** @type {string | null} what the last copy did, shown for a few seconds */
  let copied = $state(null);
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let copiedTimer;

  onMount(() => {
    dialog?.showModal();
    link.licenseKeys(button, item.id).then((read) => {
      keys = read.keys;
      failure = read.failure;
    });
    return () => clearTimeout(copiedTimer);
  });

  /** Put a key on the clipboard, to paste into the game. @param {string} value */
  async function copy(value) {
    try {
      await navigator.clipboard.writeText(value);
      copied = "コピーしました";
    } catch {
      copied = "コピーできませんでした（キーを選んでコピーしてください）";
    }
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copied = null), COPIED_MS);
  }
</script>

<dialog
  bind:this={dialog}
  class="keys"
  aria-label="シリアル番号"
  oncancel={(event) => {
    event.preventDefault();
    onclose();
  }}
  onclick={(event) => {
    if (event.target === dialog) onclose();
  }}
>
  <div class="body">
    <div class="head">
      <h2 class="title">シリアル番号: {item.name}</h2>
      <button type="button" class="close" aria-label="閉じる" onclick={onclose}>
        <Icon name="close" />
      </button>
    </div>
    {#if failure}
      <p class="failure" role="alert">{failure}</p>
    {:else if !keys}
      <p class="note" role="status"><Spinner /> DLsite から読み込んでいます…</p>
    {:else if keys.length === 0}
      <p class="note">このゲームにはシリアル番号がありません</p>
    {:else}
      {#each keys as key, index (index)}
        <div class="row">
          <div class="key">
            <span class="label">{key.label}</span>
            <span class="value">{key.value}</span>
          </div>
          <button
            type="button"
            class="copy"
            aria-label={`${key.label}をコピー`}
            onclick={() => copy(key.value)}
          >
            コピー
          </button>
        </div>
      {/each}
      <p class="note" role="status">{copied ?? ""}</p>
    {/if}
  </div>
</dialog>

<style lang="sass">
  .keys
    width: 720px
    max-width: calc(100vw - 32px)
    max-height: 80vh
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
    gap: var(--sp-3)
    padding: var(--sp-4)

  .head
    display: flex
    align-items: center
    gap: var(--sp-3)

  .title
    flex: 1
    min-width: 0
    margin: 0
    font-size: var(--fs-label)
    font-weight: 600
    overflow: hidden
    text-overflow: ellipsis
    white-space: nowrap

  .close
    display: inline-flex
    align-items: center
    justify-content: center
    flex: none
    width: 56px
    height: 56px
    border: 0
    background: transparent
    color: var(--c-on-surface)
    font-size: var(--fs-state)
    cursor: pointer

  .note,
  .failure
    display: flex
    align-items: center
    gap: var(--sp-2)
    min-height: 1.6em
    margin: 0
    font-size: var(--fs-sm)

  .note
    color: var(--c-muted)

  .failure
    color: var(--c-danger)

  .row
    display: flex
    align-items: center
    gap: var(--sp-4)

  .key
    display: flex
    flex: 1
    flex-direction: column
    min-width: 0

  .label
    color: var(--c-muted)
    font-size: var(--fs-sm)

  // Large and even, to read off group by group into games that split a key over boxes;
  // selectable, in case copying is refused.
  .value
    font-family: ui-monospace, Consolas, monospace
    font-size: 28px
    letter-spacing: 0.08em
    overflow-wrap: anywhere
    -webkit-user-select: text
    user-select: text

  .copy
    flex: none
    min-width: 120px
    min-height: var(--row-height)
    padding: 0 var(--sp-4)
    border: 1px solid var(--c-accent)
    border-radius: var(--radius-md)
    background: var(--c-accent-subtle)
    color: var(--c-accent)
    font: inherit
    font-size: var(--fs-label)
    font-weight: 600
    cursor: pointer
    touch-action: manipulation
    -webkit-tap-highlight-color: transparent

  @media (hover: hover)
    .copy:hover
      background: var(--c-hover-2)
</style>
