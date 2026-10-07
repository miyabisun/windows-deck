<script module>
  /** How long a toast stays. */
  const SHOW_MS = 3000;

  /** What a toast says, for a few seconds after each `show`. */
  export class Notice {
    /** @type {string | null} */
    text = $state(null);
    /** @type {ReturnType<typeof setTimeout> | undefined} */
    #timer;

    /** @param {string} text */
    show(text) {
      this.text = text;
      clearTimeout(this.#timer);
      this.#timer = setTimeout(() => (this.text = null), SHOW_MS);
    }

    stop() {
      clearTimeout(this.#timer);
    }
  }
</script>

<script>
  /** @type {{ notice: Notice, fixed?: boolean }} */
  let { notice, fixed = false } = $props();
</script>

<!-- Always present so that it is announced. -->
<p class="toast" class:fixed role="status">{notice.text ?? ""}</p>

<style lang="sass">
  // Over the bottom of its container, or of the screen when fixed.
  .toast
    position: absolute
    bottom: var(--sp-5)
    left: 50%
    max-width: calc(100% - 2 * var(--sp-5))
    margin: 0
    padding: var(--sp-3) var(--sp-4)
    border: 1px solid var(--c-border)
    border-radius: var(--radius-md)
    background: var(--c-surface-raised)
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.25)
    font-size: var(--fs-md)
    transform: translateX(-50%)
    pointer-events: none

    &:empty
      opacity: 0

  .fixed
    position: fixed
</style>
