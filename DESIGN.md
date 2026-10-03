---
version: alpha
name: Sumi / windows-deck
description: タッチモニターに全画面で出す操作盤のデザイン。
colors:
  primary: "#00707a"
  accent: "#00707a"
  accent-subtle: "rgba(0, 112, 122, 0.10)"
  surface: "#faf6ef"
  surface-raised: "#fffdf8"
  on-surface: "#3a2f28"
  muted: "#6f6257"
  border: "#e3d9c9"
  danger: "#9c2b1d"
  danger-subtle: "#f9e9e4"
  hover-1: "rgba(0, 112, 122, 0.08)"
  hover-2: "rgba(0, 112, 122, 0.16)"
typography:
  tile-label:
    fontFamily: system-ui
    fontSize: 18px
    fontWeight: 600
    lineHeight: 1.3
  tile-state:
    fontFamily: system-ui
    fontSize: 24px
    fontWeight: 600
    lineHeight: 1.25
  body:
    fontFamily: system-ui
    fontSize: 16px
    fontWeight: 400
    lineHeight: 1.6
  body-sm:
    fontFamily: system-ui
    fontSize: 14px
    fontWeight: 400
    lineHeight: 1.5
rounded:
  sm: 6px
  md: 8px
spacing:
  sp-1: 4px
  sp-2: 8px
  sp-3: 12px
  sp-4: 16px
  sp-5: 24px
components:
  tile:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.on-surface}"
    rounded: "{rounded.md}"
    padding: 16px
  tile-hover:
    backgroundColor: "{colors.hover-1}"
  tile-pressed:
    backgroundColor: "{colors.hover-2}"
  error-banner:
    backgroundColor: "{colors.danger-subtle}"
    textColor: "{colors.danger}"
    typography: "{typography.body-sm}"
    rounded: "{rounded.sm}"
    padding: 12px
  spinner:
    textColor: "{colors.accent}"
    size: 18px
---

# windows-deck

## Overview

マウスの前に置いたタッチモニターに全画面で出す、Stream Deck のような操作盤。
windows-link のボタンを並べ、1 回のタップで 1 つの操作を実行する。
主役はボタンと、その現在の状態である。説明や装飾でボタンの面積を削らない。

Sumi family の共通原本（rust-svelte-template の DESIGN.md、2026-10-04 参照）から派生した。
派生後はこの文書を正とし、原本の更新は差分を見て明示的に取り込む。

## 利用の流れ

- 起動すると、設定したモニター（未設定なら windows-link が報告するタッチ対応モニター）に全画面で出る。
  そのモニターが無ければ主モニターに出る。
- 主操作はボタンを押すこと。押すと windows-link が操作を実行し、ボタンが新しい状態を示す。
  タッチとマウスのクリックで同じ操作になる。キーボードでも Tab と Enter / Space で押せる。
- ボタンの状態は windows-link からの通知で更新する。Windows 側で出力先や音量を変えた場合も追従する。
- 押下中のボタンは処理中の表示になり、同じボタンへの連打は無視する。他のボタンは押せる。
- 押下が失敗したら、そのボタンに原因を出す。次にそのボタンを押すと消える。
  原因は利用者が直せる形で示す（例: 切替先のデバイスがつながっていない、対象のアプリが起動していない）。
- windows-link とつながらないときは、画面上部に接続先と再接続中であることを示し、ボタンを押せなくする。
  最後に分かっていた状態は薄く残す。つながると自動で戻り、通知を消す。
- ボタンが 0 個なら、windows-link の設定ファイルにボタンを追加するよう案内する。
- 長押しでブラウザのメニューを出さない。文字の選択やダブルタップでの拡大もしない。

## Colors

暗色は Sumi、明色は Kinari とし、OS の明暗設定に従う。手動の切り替えと保存キーは持たない
（全画面の操作盤にメニューを置かないため）。`:root` を Sumi にし、
`prefers-color-scheme: light` で Kinari にする。

| 役割 | Kinari | Sumi |
|---|---|---|
| surface | #faf6ef | #191919 |
| surface-raised | #fffdf8 | #232323 |
| on-surface | #3a2f28 | #e6e6e6 |
| muted | #6f6257 | #9a9a9a |
| border | #e3d9c9 | #333333 |
| accent / primary | #00707a | #4cc3cb |
| accent-subtle | rgba(0,112,122,.10) | rgba(76,195,203,.15) |
| danger | #9c2b1d | #ff6b6b |
| danger-subtle | #f9e9e4 | #3a1a1a |
| hover-1 | rgba(0,112,122,.08) | #2c2c2c |
| hover-2 | rgba(0,112,122,.16) | #363636 |

アクセントは原本の金色から青緑へ変え、windows-deck 固有にした。
コントラストは accent が Kinari の surface に 5.4:1、Sumi の surface に 8.4:1。
アクセントの用途はフォーカスの輪郭と処理中のスピナーだけとする。ボタンの面を塗らない。
失敗は danger の文字で示し、色だけに頼らず文で原因を書く。
押せない状態は不透明度 50% と `aria-disabled` で示す。

## Typography

書体は system-ui。操作盤は腕の長さほど離れて見るため、原本より大きい 2 役割を足した。

- ボタンの名前: 18px / 600。1 行に収まらなければ 2 行まで折り返す。
- ボタンの状態: 24px / 600。ボタンで最も目立つ文字。2 行を超える分は省略する。
- 状態の補足と失敗の原因: 14px。補足は muted、失敗は danger。
- 接続の通知と空の案内: 16px。

## Layout

- ヘッダーとメニューは置かない。画面全体をボタンの格子にする。
- 格子は幅 240px 以上の列を自動で並べ、間隔 12px、外周の余白 16px とする。
  1920×1080 で 7 列、1280×800 で 5 列になる。ボタンの高さは 144px 以上。
- ボタンの中は、上に名前、下に状態（必要なら補足）を左揃えで置く。余白は 16px。
- 接続の通知は格子の上に全幅で出す。通常時は場所を取らない。
- 縦にあふれたらページ全体を縦にスクロールする。内側にスクロール領域を作らない。

## Components

### ボタン（tile）

`<button>` とし、surface-raised、1px の border、8px の角丸。
ホバーの hover-1 はマウスのときだけ付け、押している間は hover-2 にする。
タップの後にホバーや押下の色を残さない（指を離したボタンが選択中に見えるため）。
処理中は右上に 18px のスピナーを出し、
`aria-busy="true"` にする。失敗中は border を danger にし、原因の文を出す。
`touch-action: manipulation` と `user-select: none` を付ける。

### 接続の通知（error-banner）

danger-subtle の面に danger の文字、6px の角丸、12px の余白。
`role="status"` で読み上げる。再試行は自動なので、ボタンは置かない。

### スピナー

18px、accent 色の 2px の円弧。`prefers-reduced-motion: reduce` では回転を止める。

## Elevation & Depth

影は使わない。面の濃淡と 1px の境界だけで区切る。
フォーカスは `:focus-visible` に 2px の accent 色の輪郭と 2px の間隔を付ける。

## Verification

Chromium + Playwright で、windows-link を模したサーバーに対して次を確かめる。

- ボタンの一覧が設定の順に出て、名前と状態が読める。
- 押すと処理中になり、応答の状態に変わる。失敗の原因がそのボタンに出て、次の押下で消える。
- 外部からの状態の変化（通知）がボタンに反映される。
- 接続できないと通知が出てボタンが押せなくなり、サーバーが戻ると自動で消えて押せる。
- ボタンが 0 個のときの案内。
- Sumi と Kinari の両方で、1920×1080 と 1280×800 の格子の列数、ページの横はみ出しが 0px。
