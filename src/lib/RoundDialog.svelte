<script>
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";
  import { roundSummary } from "./display.js";

  /** How often windows-link is asked again while the dialog is open. */
  const POLL_MS = 1000;

  /** @type {{ link: import("./link.svelte.js").Link, button: string, label: string, onclose: () => void }} */
  let { link, button, label, onclose } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();
  /** @type {any} windows-link's `GET …/library/update` */
  let round = $state(null);
  /** @type {string | null} */
  let failure = $state(null);
  const summary = $derived(round ? roundSummary(round) : null);

  async function read() {
    const answer = await link.libraryRound(button);
    round = answer.round ?? round;
    failure = answer.failure;
  }

  onMount(() => {
    dialog?.showModal();
    read();
    const timer = setInterval(read, POLL_MS);
    return () => clearInterval(timer);
  });
</script>

<dialog
  bind:this={dialog}
  class="round"
  aria-label="更新の状況"
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
      <h2 class="title">更新の状況: {label}</h2>
      <button type="button" class="close" aria-label="閉じる" onclick={onclose}>
        <Icon name="close" />
      </button>
    </div>
    {#if failure}
      <p class="failure" role="alert">{failure}</p>
    {:else if !summary}
      <p class="note" role="status"><Spinner /> 読み込んでいます…</p>
    {:else}
      <p class="state" role="status">
        {#if summary.busy}<Spinner />{/if}
        {summary.text}
      </p>
      {#if summary.failure}
        <p class="failure" role="alert">{summary.failure}</p>
      {/if}
      {#if round.jobs.length}
        <ul class="jobs">
          {#each round.jobs as job (job.id)}
            <li class="job">
              <span class="name">{job.name}</span>
              <span class="status" class:failed={job.status.includes("失敗")}>{job.status}</span>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
    <div class="actions">
      {#if link.failures[button]}
        <p class="failure">{link.failures[button]}</p>
      {/if}
      <button
        type="button"
        class="now"
        disabled={!!summary?.busy || !!link.pending[button] || link.status !== "connected"}
        onclick={() => link.updateLibrary(button)}
      >
        {#if link.pending[button]}<Spinner />{/if}
        今すぐ更新
      </button>
    </div>
  </div>
</dialog>

<style lang="sass">
  .round
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

  .state,
  .note,
  .failure
    display: flex
    align-items: center
    gap: var(--sp-2)
    margin: 0
    line-height: 1.5

  .state
    font-size: var(--fs-label)

  .note
    color: var(--c-muted)
    font-size: var(--fs-sm)

  .failure
    color: var(--c-danger)
    font-size: var(--fs-sm)

  .jobs
    display: flex
    flex-direction: column
    margin: 0
    padding: 0
    list-style: none
    border-top: 1px solid var(--c-border)

  .job
    display: flex
    align-items: center
    gap: var(--sp-3)
    min-height: var(--row-height)
    border-bottom: 1px solid var(--c-border)

  .name
    flex: 1
    min-width: 0
    overflow: hidden
    text-overflow: ellipsis
    white-space: nowrap

  .status
    flex: none
    max-width: 50%
    color: var(--c-muted)
    font-size: var(--fs-sm)
    text-align: right

    &.failed
      color: var(--c-danger)

  .actions
    display: flex
    align-items: center
    justify-content: flex-end
    gap: var(--sp-3)

    .failure
      flex: 1

  .now
    display: inline-flex
    align-items: center
    gap: var(--sp-2)
    flex: none
    min-height: 56px
    padding: 0 var(--sp-5)
    border: 1px solid var(--c-accent)
    border-radius: var(--radius-md)
    background: transparent
    color: var(--c-accent)
    font: inherit
    font-size: var(--fs-label)
    cursor: pointer
    touch-action: manipulation
    -webkit-tap-highlight-color: transparent

    &:disabled
      opacity: 0.5
      cursor: default

  @media (hover: hover)
    .now:not(:disabled):hover
      background: var(--c-hover-1)
</style>
