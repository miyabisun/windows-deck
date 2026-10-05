<script>
  import ContextMenu from "./ContextMenu.svelte";
  import KeysDialog from "./KeysDialog.svelte";
  import LabelPicker from "./LabelPicker.svelte";
  import ProgramChooser from "./ProgramChooser.svelte";
  import { itemKey } from "./link.svelte.js";

  /** @type {{ link: import("./link.svelte.js").Link, button: string, item: { id: string, name: string, installed?: boolean, choosable?: boolean }, pinned: boolean, x: number, y: number, onclose: () => void, onchange?: () => void, onnotice?: (text: string) => void }} */
  let {
    link,
    button,
    item,
    pinned,
    x,
    y,
    onclose,
    onchange = () => {},
    onnotice = () => {},
  } = $props();

  let picking = $state(false);
  let choosing = $state(false);
  let showingKeys = $state(false);
  // A library whose games can have license keys (DLsite) says so in its button's state.
  const keys = $derived(
    link.buttons.find((/** @type {any} */ b) => b.id === button)?.state?.license_keys === true,
  );

  // The menu closes as soon as a choice is made, so each action reads what it needs
  // from the props before it waits.
  async function pin() {
    const { id, name } = item;
    const pin = !pinned;
    const notify = onnotice;
    // A failure stays on the game's tile (Link keeps it).
    const failed = await link.setPinned(button, id, pin);
    if (!failed) notify(`「${name}」を TOP ${pin ? "に固定しました" : "から外しました"}`);
  }

  async function browse() {
    const key = itemKey(button, item.id);
    const failed = await link.openFolder(button, item.id);
    if (failed) link.failures[key] = failed;
  }
</script>

{#if picking}
  <LabelPicker {link} {button} {item} {onclose} {onchange} />
{:else if choosing}
  <ProgramChooser {link} {button} {item} {onclose} />
{:else if showingKeys}
  <KeysDialog {link} {button} {item} {onclose} />
{:else}
  <ContextMenu
    {x}
    {y}
    title={item.name}
    items={[
      { label: pinned ? "TOPから外す" : "TOPに固定", onselect: pin },
      { label: "ローカルファイル閲覧", disabled: item.installed === false, onselect: browse },
      { label: "ラベル設定", onselect: () => (picking = true) },
      ...(item.choosable
        ? [{ label: "起動ファイルを選ぶ", onselect: () => (choosing = true) }]
        : []),
      ...(keys ? [{ label: "シリアル番号", onselect: () => (showingKeys = true) }] : []),
    ]}
    onclose={() => {
      if (!picking && !choosing && !showingKeys) onclose();
    }}
  />
{/if}
