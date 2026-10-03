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

## Requirements

- Windows 10 or 11 (x64) with the WebView2 runtime (included in Windows 11)
- windows-link 0.1.1 or later running on the same PC (it allows this panel's window to
  read its API)
- To build: Rust 1.96 (selected by `rust-toolchain.toml`) with the MSVC toolchain, and
  Node.js 24

## Build and run

```powershell
npm ci
npm run tauri build -- --no-bundle
.\src-tauri\target\release\windows-deck.exe
```

During development, `npm run tauri dev` runs the panel with hot reload.

## Configuration

An optional YAML file at `%LOCALAPPDATA%\windows-deck\config.yaml`
(`WINDOWS_DECK_CONFIG` overrides the path):

```yaml
link: http://127.0.0.1:4730 # windows-link URL (default)
monitor: JAPANNEXT MNT # monitor to fill (optional)
```

`monitor` is matched against windows-link's `GET /touch-monitors` by `id`, `name` or
display name (`\.\DISPLAY2`), ignoring case. Without it the panel fills the touch
monitor, if windows-link reports exactly one. If the monitor is not connected, there is
no single touch monitor, or windows-link does not answer at startup, the panel fills the
primary monitor; in the last case it moves to the right monitor once windows-link
answers. A file that cannot be read is reported on the panel, which then uses the
defaults. Restart the panel after editing the file.

## Start at logon

Copy the build to a per-user location and register a Task Scheduler task for your own
logon (no administrator rights needed):

```powershell
$dir = "$env:LOCALAPPDATA\Programs\windows-deck"
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item .\src-tauri\target\release\windows-deck.exe $dir

$action = New-ScheduledTaskAction -Execute "$dir\windows-deck.exe"
$trigger = New-ScheduledTaskTrigger -AtLogOn -User "$env:USERDOMAIN\$env:USERNAME"
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) `
  -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
Register-ScheduledTask -TaskName windows-deck -Action $action -Trigger $trigger `
  -Settings $settings -RunLevel Limited -Force
Start-ScheduledTask -TaskName windows-deck    # start it now
```

The panel has no title bar; close it with Alt+F4 or `Stop-Process -Name windows-deck`.

## Development

```powershell
npm run format:check
npm run lint
npm run check          # svelte-check
npm test               # unit tests (Vitest)
npx playwright install chromium
npm run test:e2e       # Chromium against a stand-in windows-link
npm run build
cd src-tauri; cargo fmt --check; cargo clippy --all-targets -- -D warnings; cargo test
```

The end-to-end tests serve the built page and replace windows-link with
`e2e/mock-link.js`, so they need no Windows audio devices. In a browser (without Tauri)
the page takes the windows-link URL from `?link=`.

The design rules are in [DESIGN.md](DESIGN.md).

## License

MIT
