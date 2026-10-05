<script module>
  // 24×24 outline icons drawn with currentColor (DESIGN.md "Components").
  export const ICONS = {
    plus: "M12 5v14M5 12h14",
    moon: "M20 14.5A8.5 8.5 0 1 1 9.5 4a7 7 0 0 0 10.5 10.5z",
    close: "M6 6l12 12M18 6 6 18",
    check: "M5 12.5l4.5 4.5L19 7.5",
  };
</script>

<script>
  /** @type {{ name: keyof typeof ICONS }} */
  let { name } = $props();
</script>

<svg
  class="icon"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={ICONS[name]} />
</svg>

<style lang="sass">
  .icon
    width: 1.2em
    height: 1.2em
    flex: none
</style>
