<script>
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";

  /** @type {{ link: import("./link.svelte.js").Link, button: string, item: { id: string, name: string }, start?: boolean, onclose: () => void, onstarted?: (item: { id: string, name: string }) => void }} */
  let { link, button, item, start = false, onclose, onstarted = () => {} } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();
  /** @type {{ candidates: string[], chosen: string | null } | null} */
  let programs = $state(null);
  /** @type {string | null} */
  let failure = $state(null);
  let busy = $state(false);

  onMount(() => {
    dialog?.showModal();
    link.programs(button, item.id).then((read) => {
      programs = read.programs;
      failure = read.failure;
    });
  });

  /** Remember the program, and start the game with it when asked to. @param {string} program */
  async function choose(program) {
    if (busy) return;
    busy = true;
    const { id, name } = item;
    const started = onstarted;
    failure = await link.chooseProgram(button, id, program);
    const result = !failure && start ? await link.startItem(button, id) : null;
    busy = false;
    if (failure) return;
    onclose();
    if (result === "started") started({ id, name });
  }
</script>

<dialog
  bind:this={dialog}
  class="chooser"
  aria-label="起動ファイルを選ぶ"
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
      <h2 class="title">起動ファイルを選ぶ: {item.name}</h2>
      <button type="button" class="close" aria-label="閉じる" onclick={onclose}>
        <Icon name="close" />
      </button>
    </div>
    <p class="note">選んだファイルを覚えて、次からはそれで起動します。</p>
    {#if failure}
      <p class="failure" role="alert">{failure}</p>
    {/if}
    {#if !programs && !failure}
      <p class="note" role="status"><Spinner /> 読み込んでいます…</p>
    {:else if programs}
      <div class="rows" role="radiogroup" aria-label="起動ファイル">
        {#each programs.candidates as program (program)}
          <button
            type="button"
            role="radio"
            class="row"
            aria-checked={programs.chosen === program}
            disabled={busy}
            onclick={() => choose(program)}
          >
            <span class="mark">
              {#if programs.chosen === program}
                <Icon name="check" />
              {/if}
            </span>
            {program}
          </button>
        {/each}
      </div>
    {/if}
  </div>
</dialog>

<style lang="sass">
  .chooser
    width: 640px
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
    min-height: var(--row-height)
    padding: 0 var(--sp-2)
    border: 0
    border-radius: var(--radius-md)
    background: transparent
    color: var(--c-on-surface)
    font: inherit
    font-size: var(--fs-label)
    text-align: left
    overflow-wrap: anywhere
    cursor: pointer
    touch-action: manipulation
    -webkit-tap-highlight-color: transparent

    &:disabled
      opacity: 0.5
      cursor: default

  .mark
    display: inline-flex
    align-items: center
    justify-content: center
    flex: none
    width: 28px
    height: 28px
    border: 2px solid var(--c-border)
    border-radius: 50%

  [aria-checked="true"] .mark
    border-color: var(--c-accent)
    background: var(--c-accent-subtle)

  @media (hover: hover)
    .row:not(:disabled):hover
      background: var(--c-hover-1)
</style>
