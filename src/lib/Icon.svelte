<script module>
  // 24×24 outline icons drawn with currentColor (DESIGN.md "Components").
  export const ICONS = {
    plus: "M12 5v14M5 12h14",
    moon: "M20 14.5A8.5 8.5 0 1 1 9.5 4a7 7 0 0 0 10.5 10.5z",
    close: "M6 6l12 12M18 6 6 18",
    check: "M5 12.5l4.5 4.5L19 7.5",
    // A speaker cone: alone for the mixer, with sound or crossed out for mute.
    volume: "M11 5 6 9H2v6h4l5 4z",
    sound: "M11 5 6 9H2v6h4l5 4zM15.5 8.5a5 5 0 0 1 0 7M19 5a10 10 0 0 1 0 14",
    muted: "M11 5 6 9H2v6h4l5 4zM22 9l-6 6M16 9l6 6",
    // Output devices.
    speaker:
      "M6 2h12a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2zM12 10a4 4 0 1 0 0 8a4 4 0 1 0 0-8zM12 6h.01",
    headphones:
      "M3 18v-6a9 9 0 0 1 18 0v6M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z",
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
