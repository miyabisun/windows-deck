<script>
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";

  /** @type {{ link: import("./link.svelte.js").Link, onclose: () => void }} */
  let { link, onclose } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();
  /** @type {{ master: { volume: number, muted: boolean }, apps: Array<{ process: string, name: string, volume: number }> } | null} */
  let mixer = $state(null);
  /** @type {string | null} */
  let failure = $state(null);

  onMount(() => {
    dialog?.showModal();
    link.mixer().then((read) => {
      mixer = read.mixer;
      failure = read.failure;
    });
  });

  /** @param {number} volume 0-1 */
  const percent = (volume) => Math.round(volume * 100);

  // One request at a time per slider: a value set meanwhile waits its turn, so a slide
  // sends as fast as windows-link answers and ends where the finger stopped.
  /** @type {Record<string, { busy: boolean, next: number | null }>} */
  const queues = {};

  /**
   * @param {string} key
   * @param {number} volume 0-1
   * @param {(volume: number) => Promise<{ mixer: any, failure: string | null }>} send
   */
  async function change(key, volume, send) {
    const queue = (queues[key] ??= { busy: false, next: null });
    queue.next = volume;
    if (queue.busy) return;
    queue.busy = true;
    while (queue.next !== null) {
      const sending = queue.next;
      queue.next = null;
      const result = await send(sending);
      failure = result.failure;
      // Take windows-link's answer only once the slider has stopped.
      if (result.mixer && queue.next === null) mixer = result.mixer;
    }
    queue.busy = false;
  }

  /** @param {Event} event */
  const value = (event) =>
    Number(/** @type {HTMLInputElement} */ (event.currentTarget).value) / 100;

  /** @param {Event} event */
  function setWhole(event) {
    if (!mixer) return;
    const volume = value(event);
    // Moving the volume unmutes, as Windows' own slider does.
    mixer.master = { volume, muted: false };
    change("master", volume, (v) => link.setMaster({ volume: v }));
  }

  /** @param {{ process: string, volume: number }} app @param {Event} event */
  function setApp(app, event) {
    const volume = value(event);
    app.volume = volume;
    const { process } = app;
    change(`app:${process}`, volume, (v) => link.setAppVolume(process, v));
  }
</script>

<dialog
  bind:this={dialog}
  class="mixer"
  aria-label="ミキサー"
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
      <h2 class="title">ミキサー</h2>
      <button type="button" class="close" aria-label="閉じる" onclick={onclose}>
        <Icon name="close" />
      </button>
    </div>
    {#if failure}
      <p class="failure" role="alert">{failure}</p>
    {/if}
    {#if mixer}
      <div class="row whole">
        <span class="name">
          全体
          {#if mixer.master.muted}<span class="muted">ミュート中</span>{/if}
        </span>
        <input
          type="range"
          min="0"
          max="100"
          step="1"
          aria-label="全体"
          value={percent(mixer.master.volume)}
          style:--fill={`${percent(mixer.master.volume)}%`}
          oninput={setWhole}
        />
        <span class="value">{percent(mixer.master.volume)}%</span>
      </div>
      <div class="apps">
        {#each mixer.apps as app (app.process)}
          <div class="row">
            <span class="name" title={app.process}>{app.name}</span>
            <input
              type="range"
              min="0"
              max="100"
              step="1"
              aria-label={app.name}
              value={percent(app.volume)}
              style:--fill={`${percent(app.volume)}%`}
              oninput={(event) => setApp(app, event)}
            />
            <span class="value">{percent(app.volume)}%</span>
          </div>
        {:else}
          <p class="note">音を出しているアプリはありません</p>
        {/each}
      </div>
    {:else if !failure}
      <p class="note" role="status"><Spinner /> 読み込んでいます…</p>
    {/if}
  </div>
</dialog>

<style lang="sass">
  .mixer
    width: 960px
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
    gap: var(--sp-2)
    padding: var(--sp-4)

  .head
    display: flex
    align-items: center
    gap: var(--sp-3)

  .title
    flex: 1
    margin: 0
    font-size: var(--fs-label)
    font-weight: 600

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

  .row
    display: grid
    grid-template-columns: 200px 1fr 4.5em
    align-items: center
    gap: var(--sp-4)
    min-height: var(--row-height)

  // The whole volume heads the list, set apart from the apps below it.
  .whole
    padding-bottom: var(--sp-2)
    border-bottom: 1px solid var(--c-border)

    .name
      font-weight: 600

  .name
    min-width: 0
    font-size: var(--fs-label)
    overflow: hidden
    text-overflow: ellipsis
    white-space: nowrap

  .muted
    display: block
    color: var(--c-danger)
    font-size: var(--fs-sm)
    font-weight: 400

  .value
    font-size: var(--fs-label)
    font-variant-numeric: tabular-nums
    text-align: right

  // The whole row height takes a tap or a slide; a vertical drag still scrolls.
  input[type="range"]
    width: 100%
    height: var(--row-height)
    margin: 0
    background: transparent
    cursor: pointer
    touch-action: pan-y
    -webkit-appearance: none
    appearance: none

    &::-webkit-slider-runnable-track
      height: 12px
      border-radius: 6px
      background: linear-gradient(to right, var(--c-accent) var(--fill), var(--c-border) var(--fill))

    &::-webkit-slider-thumb
      width: 36px
      height: 36px
      margin-top: -12px
      border: 3px solid var(--c-surface-raised)
      border-radius: 50%
      background: var(--c-accent)
      -webkit-appearance: none
      appearance: none

    &:focus-visible
      outline: 2px solid var(--c-accent)
      outline-offset: 2px
</style>
