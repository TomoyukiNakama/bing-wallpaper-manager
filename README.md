# Bing Wallpaper Manager

Ubuntu (GNOME) 用の Bing 壁紙アプリです。Rust と [gpui-kit](https://gpui-kit.com) で作っています。

- **ダッシュボード**: 選んだ写真を上部に大きく表示し、その下に「今日の写真」と「過去の写真」（日付ごと）を並べます
- 「今日の写真」は 6 地域（日本・米国・英国・ドイツ・フランス・中国）の最新を、同じ写真ごとにまとめて配信地域を添えます
- 写真をクリックすると上部に大きく表示され、「壁紙にする」で GNOME の壁紙（ライト／ダーク両方）に設定します
- 配色はシステムのライト／ダークに合わせて切り替わります
- **過去の写真**: Bing 公式 API で 15 日前までを取得します。さらに古い写真は、一覧の末尾の「さらに古い写真を読み込む」で、有志のアーカイブ（[Bing-Wallpaper-Archive](https://github.com/npanuhin/Bing-Wallpaper-Archive)）から約 1 か月分ずつ読み込めます（米国は 2010 年から、ほかの地域は 2024 年 4 月から）。古い写真はサムネイルだけを保存し、壁紙用の画像は「壁紙にする」ときにダウンロードします
- **設定ダイアログ**（右上の「設定」、Ctrl+,）: 一般（言語・テーマ）／壁紙（表示方法）／保存先／このアプリについて（バージョン・ライセンス・使用ライブラリ）
- **多言語対応**: 日本語と英語。自動のときは OS の言語（`LANG`）に合わせます。英語表示では、英語圏の地域で配信された見出しと説明を優先して表示します
- **壁紙の表示方法**（設定の「壁紙」）: 写真とモニターの縦横比が違うときの合わせ方を選べます。モニターの比率でのプレビュー付きです
  - 画面いっぱいに拡大（既定）／写真全体を表示／引き伸ばす／中央に元の大きさで表示／複数のモニターにまたがる
  - 今の壁紙がこのアプリの写真なら、選んだ時点で反映されます
- 写真を右クリック、または大きな写真の「…」から「画像を開く」「フォルダで表示」ができます
- キーボード操作: ←→ で写真を選ぶ、Enter で壁紙にする、F5 / Ctrl+R で更新、Ctrl+, で設定
- **保存先**は「設定」の「変更…」で選べます（既定は `~/Pictures/BingWallpaper`、日本語環境では `~/ピクチャ/BingWallpaper`）。設定は `~/.config/bing-wallpaper-manager/config.json` に保存されます
- 取得した画像（全地域の直近 8 日分）は UHD で保存先に保管されます。同じ写真は地域や配信日が違っても 1 枚だけ保存し、保管済みの画像は再ダウンロードしません。日が変わって最新でなくなった画像は「過去の画像」に並びます
- `--apply` オプションを付けると、GUI を開かずに今日の画像を壁紙に設定します（保存先は GUI と共通）

### 保存先の構成

```text
<保存先>/
  20261108_OHR.Foo_UHD.jpg            壁紙用の画像
  .bing-wallpaper/meta/OHR.Foo.json   タイトル・配信地域などのメタデータ
  .bing-wallpaper/thumbs/OHR.Foo.jpg  ダッシュボード用サムネイル (640x360)
```

## ビルド

gpui-kit が依存する gpui は、Ubuntu 26.04 標準の rustc 1.93 ではビルドできません。rustup で新しい stable を入れてください。
`.cargo/config.toml` で rustc を rustup の stable に固定しているので、`cargo` 自体は Ubuntu 標準のままで構いません。

```sh
# rustup が無い場合（PATH の変更は不要）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path

cargo run --release
```

`.cargo/config.toml` の rustc のパスは `/home/tn/...` 固定です。別のユーザーや別のマシンでビルドするときは書き換えてください。

## コマンドラインで使う

```sh
bing-wallpaper-manager --apply              # ja-JP の今日の画像を設定
bing-wallpaper-manager --apply --mkt en-US  # 地域を指定
```

### 毎日自動で更新する（systemd user timer）

`~/.config/systemd/user/bing-wallpaper.service`:

```ini
[Unit]
Description=Set Bing wallpaper
After=network-online.target

[Service]
Type=oneshot
ExecStart=%h/.local/bin/bing-wallpaper-manager --apply
```

`~/.config/systemd/user/bing-wallpaper.timer`:

```ini
[Unit]
Description=Daily Bing wallpaper

[Timer]
OnStartupSec=1min
OnCalendar=*-*-* 09:05
Persistent=true

[Install]
WantedBy=timers.target
```

```sh
install -Dm755 target/release/bing-wallpaper-manager ~/.local/bin/bing-wallpaper-manager
systemctl --user daemon-reload
systemctl --user enable --now bing-wallpaper.timer
```

## 見た目の確認（スクリーンショット）

仮想の Wayland ディスプレイ（sway の headless モード）でアプリを動かし、PNG を撮ります。
今のデスクトップ・設定・保存先には触れません（撮影用の設定と画像は `target/screenshot-fixture/` に置かれます）。

```sh
sudo apt install sway grim          # 初回のみ
cargo build --release
scripts/screenshot.sh target/shot.png            # 既定: 15 秒待って 1440x960 で撮影
BWM_THEME=dark scripts/screenshot.sh target/dark.png 10 1280x1300
```

`BWM_THEME=light|dark` は通常の起動でも使え、システムの外観に関係なく配色を固定します。

`ACTIONS` にクリック・右クリック・スクロール・キー入力を書くと、撮影前に操作できます。仮想マウスと仮想キーボードには `tools/vpointer`（初回に自動でビルド）を使います。撮影中の壁紙設定はメモリ上の仮の保存先に書かれ、実際の壁紙は変わりません。

```sh
ACTIONS=$'scroll down 8 700 800\nclick 880 830\nshot target/step1.png' \
  scripts/screenshot.sh target/step2.png 10 1280x1000
```

## 構成

| ファイル | 役割 |
| --- | --- |
| `src/main.rs` | gpui-kit のダッシュボード UI と CLI |
| `src/bing.rs` | Bing の HPImageArchive API、画像ダウンロード |
| `src/library.rs` | 保存先への画像の保管と読み込み |
| `src/config.rs` | 設定ファイル（保存先） |
| `src/theme.rs` | 配色（ライト／ダーク）とフォント |
| `src/settings.rs` | 設定ダイアログ（一般・壁紙・保存先・このアプリについて） |
| `src/i18n.rs` | 表示言語（`tr!("日本語", "English")`） |
| `src/archive.rs` | 有志のアーカイブからの古い写真の一覧 |
| `src/third_party.tsv` / `THIRD_PARTY_LICENSES.md` | 使用ライブラリの一覧（`scripts/gen-third-party.py` で生成） |
| `scripts/screenshot.sh` | 仮想ディスプレイでのスクリーンショット撮影 |
| `tools/vpointer/` | 撮影時にクリック・スクロールを送る仮想マウス（別プロジェクト） |
| `src/wallpaper.rs` | `gsettings` による GNOME の壁紙設定 |

## ライセンス

MIT OR Apache-2.0。使用しているライブラリの一覧は [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) にあります（依存を変えたら `python3 scripts/gen-third-party.py` で作り直す）。
