<svg class="spinner" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
  <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" stroke-width="2" opacity="0.25" />
  <path
    d="M21 12a9 9 0 0 0-9-9"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
  />
</svg>

<style lang="sass">
  .spinner
    color: var(--c-accent)
    animation: spin 0.8s linear infinite
    flex: none

  @keyframes spin
    to
      transform: rotate(360deg)

  @media (prefers-reduced-motion: reduce)
    .spinner
      animation: none
</style>
