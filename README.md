# windows-deck

A Stream Deck style touch panel for Windows. It fills a monitor (meant for a small touch
monitor in front of the mouse) with large buttons and presses them on
[windows-link](https://github.com/miyabisun/windows-link), the resident server that does
the actual work: switching the audio output, toggling an application's volume and so on.

The panel holds no button definitions of its own. It shows the buttons windows-link has,
in their order, with their current state, and follows changes through windows-link's
`/events` socket. Tap, click, or use Tab and Enter.

- A failed press shows why on that button (for example, the device to switch to is not
  connected) until the button is pressed again.
- While windows-link cannot be reached, a notice says so, the buttons cannot be pressed,
  and the panel reconnects by itself.
- The theme follows the Windows light/dark setting.

## Virtual desktops

The panel stays on every Windows virtual desktop: once connected (and again after every
reconnection) it asks windows-link to pin its window. A row of tabs at the top shows the
desktops, with the current one selected. Switching desktops in Windows moves the
selection; tapping a tab switches Windows to that desktop. Renamed, added and removed
desktops show up right away.

Each tab shows windows-link's shared buttons (except those that list the desktop in
`except`) followed by the buttons of the desktop's own file, `desktops/<desktop name>.yaml`
in windows-link's configuration folder. If windows-link cannot reach the virtual desktops,
there are no tabs and every button is shown.

At the right end of the tab row:

- **+** lists the desktop files that have no desktop (for example after the desktop was
  renamed or removed in Windows); picking one creates that desktop and switches to it.
- **Moon** puts the PC to sleep right away.

Buttons with an icon in windows-link (an exe's or a shortcut's Windows icon) show it next
to their label.

## Game library

A windows-link `steam.library` button opens your Steam library in a dialog covering 80% of
the screen:

- Type in the search field at the top to narrow the games by name, like fzf: games that
  contain every word come first, then those where each word's letters appear in order
  (full-width and half-width letters and case do not matter).
- The chips below it are your labels (Steam's favorites and hidden, and your own
  collections); tap them to narrow the list (a game in any selected label shows). The
  first chip, **ラベル非登録**, stands for games without any label and is selected when the
  dialog opens, so the games still to sort come first. Games in
  a label the button hides (`hide` in windows-link, for example `非表示`) show only while
  that chip is selected.
- **+** at the end of the chips makes a label. Long-press a chip to rename or delete it
  (deleting asks first and keeps the games; Steam's own labels cannot be renamed or
  deleted).
- Drag a game onto a label chip with the mouse to put it in that label (a finger's long
  press keeps opening the menu).
- Tap a game to start it (or bring it to the front when it runs; a game that is not
  installed opens Steam's install dialog). The dialog closes.
- Long-press (or right-click) a game for its menu: **TOPに固定** pins it to the tab, where pinned games
  follow the tab's buttons as tiles with the game's picture; **ローカルファイル閲覧** opens
  its install folder; **ラベル設定** lists the labels with a check for each, to put the game
  in or take it out, and makes a new one. A pinned tile starts its game when tapped, and
  its long-press menu has **TOPから外す**.

Changing labels needs windows-link to reach the Steam client (see windows-link's README,
"Labels"); until then the panel says why and leaves the label controls off.

Esc, tapping outside the dialog, or its close button closes it. When windows-link cannot
list the games you own (for example without a Steam Web API key) the dialog lists the
installed ones and says why. Enter in the search field starts the first game found.

A `dlsite.library` button opens the DLsite games in DLsiteNest's folders the same way.
Their pictures are the works' DLsite art (or, for games windows-link has not matched to a
work, their program icons), shown whole, and the maker shows under the title and can be
searched. A game with several programs asks once which one starts it
(the choice is remembered); its menu has **起動ファイルを選ぶ** to change it. Its labels are
kept by windows-link, so they work without Steam.

## Requirements

- Windows 10 or 11 (x64) with the WebView2 runtime (included in Windows 11)
- windows-link 0.1.3 or later running on the same PC (it allows this panel's window to
  read its API); the Steam library needs 0.1.6 or later, the DLsite library 0.1.9 or later (0.1.10 for its art)
- To build: Rust 1.96 (selected by `rust-toolchain.toml`) with the MSVC toolchain, and
  Node.js 24

## Install

Download `windows-deck-x86_64-pc-windows-msvc.exe` from the
[latest release](https://github.com/miyabisun/windows-deck/releases/latest) and save it as
`%LOCALAPPDATA%\Programs\windows-deck\windows-deck.exe`, then register it to
[start at logon](#start-at-logon). From PowerShell:

```powershell
$dir = "$env:LOCALAPPDATA\Programs\windows-deck"
New-Item -ItemType Directory -Force $dir | Out-Null
Invoke-WebRequest -OutFile "$dir\windows-deck.exe" `
  https://github.com/miyabisun/windows-deck/releases/latest/download/windows-deck-x86_64-pc-windows-msvc.exe
```

Only a copy in that folder [updates itself](#updates). Each release also has a `.sha256`
file to check the download against. Windows 10 may need the
[WebView2 runtime](https://developer.microsoft.com/microsoft-edge/webview2/) (the
"Evergreen Bootstrapper"); Windows 11 already has it.

## Build and run

```powershell
npm ci
npm run tauri build -- --no-bundle
.\src-tauri\target\release\windows-deck.exe
```

During development, `npm run tauri dev` runs the panel with hot reload. Only one panel
runs at a time: starting it again brings the running one to the front.

`windows-deck --version` prints the version. The release build writes its log to
`%LOCALAPPDATA%\windows-deck\windows-deck.log`.

## Configuration

An optional YAML file at `%LOCALAPPDATA%\windows-deck\config.yaml`
(`WINDOWS_DECK_CONFIG` overrides the path):

```yaml
link: http://127.0.0.1:4730 # windows-link URL (default)
monitor: JAPANNEXT MNT # monitor to fill (optional)
```

`monitor` is matched against windows-link's `GET /touch-monitors` by `id`, `name` or
display name (`\\.\DISPLAY2`), ignoring case. Without it the panel fills the touch
monitor, if windows-link reports exactly one. If the monitor is not connected, there is
no single touch monitor, or windows-link does not answer at startup, the panel fills the
primary monitor; in the last case it moves to the right monitor once windows-link
answers. A file that cannot be read is reported on the panel, which then uses the
defaults. Restart the panel after editing the file.

## Start at logon

Register a Task Scheduler task that starts the [installed](#install) exe when you sign
in. The trigger is limited to your own logon, so no administrator rights are needed:

```powershell
$dir = "$env:LOCALAPPDATA\Programs\windows-deck"
$action = New-ScheduledTaskAction -Execute "$dir\windows-deck.exe"
$trigger = New-ScheduledTaskTrigger -AtLogOn -User "$env:USERDOMAIN\$env:USERNAME"
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) `
  -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
Register-ScheduledTask -TaskName windows-deck -Action $action -Trigger $trigger `
  -Settings $settings -RunLevel Limited -Force
Start-ScheduledTask -TaskName windows-deck    # start it now
```

The panel has no title bar; close it with Alt+F4 or `Stop-Process -Name windows-deck`.

## Updates

The installed panel checks the latest GitHub release when it starts and every hour. When
the release is newer than itself it updates without asking, the same way windows-link
does:

1. downloads the exe and its `.sha256` file over HTTPS and checks the SHA-256,
2. saves it as `windows-deck.exe.new` and checks that it runs and reports the release's
   version,
3. renames the running exe to `windows-deck.exe.old` (Windows allows renaming a running
   exe, not replacing it) and moves the new one into its place,
4. starts the new exe and exits. The new panel waits for the old one to close, and
   deletes `windows-deck.exe.old` once its window is shown.

There is no signing key: the download is trusted through GitHub, HTTPS and the SHA-256
published with the release, so a fork can publish its own releases the same way. Any
failure leaves the running version as it is and is logged; the next check tries again.
Debug builds and copies run from anywhere other than
`%LOCALAPPDATA%\Programs\windows-deck` never update. Nothing needs administrator rights.

To check right away instead of waiting for the hour, run
`%LOCALAPPDATA%\Programs\windows-deck\windows-deck.exe --check-update` while the panel is
running; the running panel does the check (see the log for the result).
`WINDOWS_DECK_UPDATE_URL` points the panel at another release feed in the GitHub API
format (plain HTTP only for `127.0.0.1`, `localhost` and `[::1]`, for testing).

Releases are built by GitHub Actions when a `vX.Y.Z` tag is pushed
(`.github/workflows/release.yml`); the tag must match the version in `package.json`. The
workflow uses only the token GitHub provides.

## Development

```powershell
npx playwright install chromium   # once, for the end-to-end tests
git config core.hooksPath .githooks  # once per clone: the gate below
npm run verify
```

`npm run verify` runs every check: formatting (Prettier, rustfmt), ESLint, svelte-check,
the unit tests (Vitest), the end-to-end tests, and Clippy and the Rust tests in
`src-tauri`. There is no CI that tests: the `pre-push` hook in `.githooks` runs it before
`main` or a release tag is pushed, and refuses the push when anything fails (or when the
commit being pushed is not the clean working tree). Other branches are pushed without it.
GitHub Actions only builds releases.

The end-to-end tests serve the built page and replace windows-link with
`e2e/mock-link.js`, so they need no Windows audio devices. In a browser (without Tauri)
the page takes the windows-link URL from `?link=`.

The design rules are in [DESIGN.md](DESIGN.md).

## License

MIT
