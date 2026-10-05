<script>
  import { onMount } from "svelte";
  import AskDialog from "./AskDialog.svelte";
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";
  import { labelsLockedReason } from "./library.js";

  /** @type {{ link: import("./link.svelte.js").Link, button: string, item: { id: string, name: string }, onclose: () => void, onchange?: () => void }} */
  let { link, button, item, onclose, onchange = () => {} } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();
  /** @type {any} */
  let library = $state(null);
  /** @type {string | null} */
  let failure = $state(null);
  /** @type {string | null} the label being changed */
  let busy = $state(null);
  let naming = $state(false);

  const chosen = $derived(
    new Set(library?.items.find((/** @type {any} */ i) => i.id === item.id)?.labels ?? []),
  );
  const locked = $derived(labelsLockedReason(library?.labels_locked ?? null));

  async function load() {
    const read = await link.library(button);
    library = read.library;
    failure = read.failure;
  }

  onMount(() => {
    dialog?.showModal();
    load();
  });

  /** @param {string} label */
  async function toggle(label) {
    if (busy || locked) return;
    busy = label;
    failure = await link.setLabel(button, label, item.id, !chosen.has(label));
    if (!failure) {
      await load();
      onchange();
    }
    busy = null;
  }

  /** @param {string} name */
  async function create(name) {
    naming = false;
    busy = "new";
    const made = await link.createLabel(button, name);
    failure = made.failure;
    if (made.label) {
      failure = await link.setLabel(button, made.label.id, item.id, true);
      await load();
      onchange();
    }
    busy = null;
  }
</script>

<dialog
  bind:this={dialog}
  class="picker"
  aria-label="ラベル設定"
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
      <h2 class="title">ラベル設定: {item.name}</h2>
      <button type="button" class="close" aria-label="閉じる" onclick={onclose}>
        <Icon name="close" />
      </button>
    </div>
    {#if locked}
      <p class="note">{locked}</p>
    {/if}
    {#if failure}
      <p class="failure" role="alert">{failure}</p>
    {/if}
    {#if !library && !failure}
      <p class="note" role="status"><Spinner /> 読み込んでいます…</p>
    {:else if library}
      <div class="rows" role="group" aria-label="ラベル">
        {#each library.labels as label (label.id)}
          <button
            type="button"
            role="checkbox"
            class="row"
            aria-checked={chosen.has(label.id)}
            aria-busy={busy === label.id}
            disabled={!!locked}
            onclick={() => toggle(label.id)}
          >
            <span class="box">
              {#if chosen.has(label.id)}
                <Icon name="check" />
              {/if}
            </span>
            {label.name}
            {#if busy === label.id}
              <Spinner />
            {/if}
          </button>
        {/each}
        <button
          type="button"
          class="row add"
          disabled={!!locked}
          aria-busy={busy === "new"}
          onclick={() => (naming = true)}
        >
          <span class="box plain"><Icon name="plus" /></span>
          新しいラベル
        </button>
      </div>
    {/if}
  </div>
</dialog>

{#if naming}
  <AskDialog
    title="新しいラベル"
    value=""
    confirm="作って付ける"
    onconfirm={create}
    oncancel={() => (naming = false)}
  />
{/if}

<style lang="sass">
  .picker
    width: 560px
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
    margin: 0
    font-size: var(--fs-sm)

  .note
    color: var(--c-muted)

  .failure
    color: var(--c-danger)

  .rows
    display: flex
    flex-direction: column

  .row
    display: flex
    align-items: center
    gap: var(--sp-3)
    min-height: 56px
    padding: 0 var(--sp-2)
    border: 0
    border-radius: var(--radius-md)
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

  .box
    display: inline-flex
    align-items: center
    justify-content: center
    flex: none
    width: 28px
    height: 28px
    border: 2px solid var(--c-border)
    border-radius: var(--radius-sm)

  [aria-checked="true"] .box
    border-color: var(--c-accent)
    background: var(--c-accent-subtle)

  .plain
    border: 0

  @media (hover: hover)
    .row:not(:disabled):hover
      background: var(--c-hover-1)
</style>
