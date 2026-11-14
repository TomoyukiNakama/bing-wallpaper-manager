<img src="assets/icons/hicolor/128x128/apps/bing-wallpaper-manager.png" alt="" width="96" align="right">

# Bing Wallpaper Manager

[English](README.md) | **日本語**

Bing の日替わり写真を見比べて、GNOME の壁紙にする Ubuntu 用のデスクトップアプリです。Rust と [GPUI](https://gpui.rs)、[gpui-kit](https://gpui-kit.com) で作っています。

![選んだ写真が角の丸いウィンドウの上部いっぱいに広がり、マウスを乗せるとタイトルバーが重なって表示される（日本語表示・ダークテーマ）](docs/screenshot.ja.jpg)

選んだ写真を、枠のない角の丸いウィンドウの上部いっぱいに表示します。写真にマウスを乗せるとタイトルバーが現れます。壁紙にした写真には「壁紙に設定中」と表示されます。

## 機能

- **ダッシュボード**: 選んだ写真を上部に大きく表示し、その下に「今日の写真」と「過去の写真」（日付ごと）を並べます。大きな写真は上部に固定され、一覧をスクロールすると細い帯になり、写真を選ぶか細い帯をクリックすると元の広さに戻ります。写真の切り替えは、新しい画像の読み込みが終わってから、ゆっくり重ねて切り替わります
- **枠のないウィンドウ**: 角を丸くし、大きな写真をウィンドウの上端まで広げます。タイトルバー（検索・お気に入り・更新・設定・全画面表示と、ウィンドウのボタン）は写真にマウスを乗せている間だけ写真に重ねて表示します。写真をドラッグするとウィンドウを移動でき、上端をダブルクリックすると最大化します
- **今日の写真**: 選んだ地域の最新を、同じ写真ごとにまとめて配信地域を添えます。最初は 6 地域（日本・米国・英国・ドイツ・フランス・中国）で、設定でカナダ（英語・フランス語）・インド・スペイン・イタリア・ブラジルを追加できます。起動したままのときは、1 時間ごとに新しい写真を確かめます
- **壁紙にする**: GNOME の壁紙（ライト／ダーク両方）に設定します。今の壁紙には「壁紙に設定中」と表示されます
- **お気に入り**: 写真の ☆ ボタン、右クリックメニュー、Ctrl+D で登録できます。お気に入りには ★ の印が付き、タイトルバーの ★ ボタンでお気に入りだけを表示できます（検索と組み合わせることもできます）
- **過去の写真**: 保存済みの写真は、スクロールに合わせて自動で続きを表示します。Bing 公式 API で 15 日前までを取得します。さらに古い写真は、一覧の末尾の「さらに古い写真を読み込む」で、有志のアーカイブ（[Bing-Wallpaper-Archive](https://github.com/npanuhin/Bing-Wallpaper-Archive)）から約 1 か月分ずつ読み込めます（米国は 2010 年から、ほかの地域は 2024 年 4 月から）。設定の「一般」→「古い写真の読み込み」で、ボタンを押す代わりに、一覧の最後までスクロールしたときに自動で読み込むようにもできます。古い写真はサムネイルだけを保存し、壁紙用の画像は「壁紙にする」ときにダウンロードします
- **検索**（Ctrl+F）: 保存した写真を探せます
  - 言葉で探す: 題名・撮影地・解説文・配信地域・日付（`2026-11-08` など）から探します。保存した全言語の見出しと解説文が対象なので、「ペンギン」「penguin」「Pinguin」のどれでも見つかります
  - 色で探す: 赤・橙・黄・緑・水色・青・紫・ピンク・白・黒から選ぶと、その色を多く含む写真から順に並びます。言葉と組み合わせることもできます
  - Escape で検索を閉じます
- **壁紙の表示方法**: 写真とモニターの縦横比が違うときの合わせ方を、モニターの比率でのプレビューを見ながら選べます（画面いっぱいに拡大／写真全体を表示／引き伸ばす／中央に元の大きさで表示／複数のモニターにまたがる）
- **写真の情報**（大きな写真の ⓘ、Ctrl+I）: 題名・撮影地・配信日・配信地域と、Bing の解説文を横のパネルに表示します。「Bing で調べる」「題名と撮影地をコピー」もここから使えます
- **写真の操作**: 写真を右クリックするか、大きな写真の「…」ボタンから、「壁紙にする」「お気に入りに追加」「写真の情報」「Bing で調べる」「題名と撮影地をコピー」「画像を開く」「フォルダで表示」「削除」ができます
- **年月へ移動**: 過去の写真を、選んだ年月までスクロールします
- **削除**（Delete キー）: 確認のあと、写真を保存先から削除します。削除した写真は取得し直しません
- **自動更新**: 設定のスイッチを入れると、アプリを閉じていても壁紙が自動で変わります。新しい「今日の写真」にするか、お気に入り・保存した写真から決めた間隔で 1 枚ずつ選ぶかを選べます
- **設定ダイアログ**（右上の ☰、Ctrl+,）: 一般（言語・テーマ）、地域、壁紙（表示方法）、自動更新、保存先（フォルダと使用量）、このアプリについて（バージョン・ライセンス・使用しているライブラリ）
- **多言語対応**: 日本語・英語・中国語（簡体字／繁体字）・韓国語・ドイツ語・フランス語・スペイン語・イタリア語・ポルトガル語・ロシア語の 11 言語。自動のときは OS の言語（`LANG`）に合わせます。写真の見出しと説明は、その言語の地域で配信されたものを優先して表示します
- **テーマ**: システムに合わせる／ライト／ダーク
- **キーボード操作**: ←→ で写真を選ぶ、Enter で壁紙にする、Ctrl+D でお気に入りに追加・解除、Ctrl+I で写真の情報、Delete で削除、Home / End で一覧の最初・最後へ、Page Up / Page Down で 1 画面ずつスクロール、F5 / Ctrl+R で更新、Ctrl+F で検索、Ctrl+, で設定、F11 で全画面表示
- **コマンドライン**: `--apply` で GUI を開かずに今日の写真（`--random` を付けると保存した写真）を壁紙にできます
- **ウィンドウは 1 つ**: もう一度起動すると、新しいウィンドウを開かずに、開いているウィンドウを前に出します。ウィンドウは前回の大きさで開きます

## 写真を見る

一覧をスクロールすると、大きな写真が細い帯に縮み、写真を並べる場所が広がります。写真を選ぶか帯をクリックすると元の広さに戻ります。過去の写真は日付ごとにまとまり、お気に入りには ★ の印が付きます。写真を右クリック（または大きな写真の「…」）すると、「壁紙にする」「お気に入りに追加」「写真の情報」「Bing で調べる」「題名と撮影地をコピー」「画像を開く」「フォルダで表示」「削除」ができます。

![一覧をスクロールして上部が細い帯になった状態と、写真の右クリックメニュー](docs/browse.ja.jpg)

### 年月へ移動

過去の写真が 2 か月分以上あると、「過去の写真」の右に **年月へ移動** が出ます。保存している年月が並び（複数の年にまたがるときは年ごとにまとまります）、選ぶとその月の最初の写真まで一覧がスクロールします。

![保存している年月が並んだ「年月へ移動」のメニュー](docs/jump.ja.jpg)

### 写真を削除する

写真のメニューの **削除…** を選ぶか、上部に表示している写真で Delete キーを押します。確認のあと、画像・サムネイル・写真の情報を保存先から削除します。削除した写真は覚えておくので、更新や古い写真の読み込みで戻ってくることはありません。今日の写真と、今の壁紙は削除できません。

![写真を削除する前の確認](docs/delete.ja.jpg)

## 写真の情報

大きな写真の **ⓘ** を押す（または **Ctrl+I**、写真のメニューの「写真の情報」）と、横のパネルに題名・撮影地と著作権表示・配信日・配信地域と、Bing の解説文が表示されます。解説文は表示言語に合うものを優先します。最近の写真の解説文は写真と一緒に取得し、古い写真の解説文は有志のアーカイブから補います。「Bing で調べる」は Bing のその写真のページをブラウザーで開き、「題名と撮影地をコピー」はクリップボードにコピーします。Escape でパネルを閉じます。

![写真の解説文を表示した「写真の情報」のパネル](docs/info.ja.jpg)

## 検索

**Ctrl+F**（またはタイトルバーの 🔍 ボタン）で、保存した写真を探せます。検索中は一覧が条件に合う写真だけになり、写真をクリックすると上部に大きく表示されます。Escape で元の一覧に戻ります。

### 言葉で探す

題名・撮影地・解説文・配信地域・日付（`2026-11-08` など）に含まれる言葉で探します。写真の見出しと解説文は配信されたすべての言語で保存しているので、言語をまたいで見つかります。下の画像は、日本語表示のまま英語の `autumn` で探した例です。日本の題名「秋色の森を水上散歩」などの写真が見つかっています。

![「autumn」で探した例](docs/search-words.ja.jpg)

### 色で探す

赤・橙・黄・緑・水色・青・紫・ピンク・白・黒の 10 色から選ぶと、その色を多く含む写真から順に並びます。色の割合は、各写真のサムネイルから裏で計算しています。言葉と組み合わせて「mountain ＋ 青」のように探すこともできます。

![青を多く含む写真](docs/search-color.ja.jpg)

## 壁紙の表示方法

写真とモニターの縦横比が違うときの合わせ方を、**設定**（右上の ☰、または Ctrl+,）の **壁紙** で選べます。プレビューはモニターの縦横比と、上部に表示中の写真で描きます。「写真全体を表示」では、余った部分は GNOME の背景色で塗られます。今の壁紙にもすぐ反映されます。

![「写真全体を表示」をプレビューしている壁紙の設定](docs/settings-fit.ja.jpg)

## 地域

Bing は、地域によって違う写真・題名・解説文を配信しています。**設定** → **地域** で、ダッシュボードに出す地域を選べます。最初は日本・米国・英国・ドイツ・フランス・中国で、カナダ（英語）・カナダ（フランス語）・インド・スペイン・イタリア・ブラジルを追加できます。少なくとも 1 つは残ります。Bing が世界共通の写真しか配信していない地域は、一覧にありません。

![Bing の地域ごとのチェックボックスが並んだ地域の設定](docs/regions.ja.jpg)

## 自動更新

**設定** → **自動更新** のスイッチを入れると、アプリを閉じていても壁紙が自動で変わります。壁紙にする写真は次から選べます。

- **今日の写真**: 1 時間ごと（とログインの少しあと）に確かめ、選んだ地域で新しい写真が配信されたら壁紙にします。自分で選んだ壁紙は、次の写真が配信されるまで残ります
- **お気に入りから選ぶ**: 30 分・1 時間・3 時間・6 時間・1 日ごとに、お気に入りから毎回違う 1 枚を選びます
- **保存した写真から選ぶ**: 同じ間隔で、保存したすべての写真から選びます。古い写真は、そのときに画像をダウンロードします

スイッチを入れると、このアプリを `--apply` 付きで実行する systemd のユーザータイマー（`~/.config/systemd/user/bing-wallpaper-manager.timer` と `.service`）を作り、切ると消します。タイマーは今のアプリの場所を実行するので、アプリを別の場所に移したときは、スイッチを一度切って入れ直してください。

![スイッチを入れた自動更新の設定](docs/auto.ja.jpg)

## 使用している技術

画面は、次の 2 つのオープンソースプロジェクトを使って作っています。

- **[GPUI](https://gpui.rs)**: [Zed Industries](https://zed.dev) がエディタ Zed のために開発している、GPU で描画する UI フレームワーク（[ソース](https://github.com/zed-industries/zed)、Apache-2.0）。このアプリは crates.io で `gpui-pre` として公開されているスナップショットを使っています
- **[gpui-kit](https://gpui-kit.com)**: [Longbridge](https://github.com/longbridge) による GPUI 用の部品集とツールキット（[ソース](https://github.com/longbridge/gpui-kit)、Apache-2.0）。ボタン・ダイアログ・メニュー・入力欄・ドロップダウン・スクロールバー・タイトルバー・テーマは、その `gpui-component` と `gpui-base` を使っています

両プロジェクトに感謝します。そのほかのライブラリは [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) にまとめています。

## インストール

### 動作環境

- GNOME デスクトップの Ubuntu（Ubuntu 26.04 で開発。Wayland と X11 のどちらでも動きます）。壁紙は GNOME の `gsettings` で設定します
- Vulkan に対応したグラフィックスドライバー（通常の Ubuntu デスクトップには入っています）

### ビルド済みのパッケージ（x86_64）

各[リリース](https://github.com/TomoyukiNakama/bing-wallpaper-manager/releases/latest)に、Ubuntu 26.04 以降（glibc 2.43 以降）向けのビルド済みパッケージがあります。`bing-wallpaper-manager-<バージョン>-x86_64-linux.tar.gz` をダウンロードして、`~/.local` に展開します。

```sh
tar -xzf bing-wallpaper-manager-*-x86_64-linux.tar.gz -C ~/.local --strip-components=1
```

ソースからビルドする場合は、次の手順に進んでください。

### 1. ビルドに必要なパッケージを入れる

```sh
sudo apt install git curl build-essential
```

フォント処理のライブラリの一部を C からビルドするため、C コンパイラ（`build-essential`）が必要です。

### 2. Rust を入れる

[rustup](https://rustup.rs) で最新の stable の Rust を入れます。Ubuntu 26.04 に含まれる Rust（1.93）は古く、GPUI をビルドできません。

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"   # または新しいターミナルを開く
```

PATH の `cargo` を Ubuntu 標準のままにしておきたい場合も、そのままビルドできます。`.cargo/config.toml` で cargo が `scripts/rustc-stable` を使うようにしてあり、これが rustup の stable の rustc を探して実行します（rustup が無ければ PATH 上の `rustc` を使います）。

### 3. ソースを取得する

```sh
git clone https://github.com/TomoyukiNakama/bing-wallpaper-manager.git
cd bing-wallpaper-manager
```

### 4. ビルドしてインストールする

```sh
scripts/install.sh
```

リリース版をビルドし、アプリ・アイコン・デスクトップ項目を `~/.local` に入れます。GNOME のアプリ一覧に「Bing 壁紙」が出ます（すぐに出ないときは、一度ログアウトしてログインし直してください）。`~/.local/bin` が PATH に入っていれば、ターミナルから `bing-wallpaper-manager` でも起動できます。初回のビルドは依存ライブラリをすべてダウンロードしてコンパイルするため、数分かかります。

別の場所に入れるときは、`PREFIX=/opt/bing-wallpaper scripts/install.sh` のように `PREFIX` を指定します。

### 更新する

```sh
git pull
scripts/install.sh
```

`scripts/install.sh` は毎回ビルドしてから入れるので、`git pull` のあとに実行すれば新しい版になります。

### アンインストール

```sh
scripts/install.sh --uninstall
```

アプリ・アイコン・デスクトップ項目を削除します。設定（`~/.config/bing-wallpaper-manager/`）と保存した写真（保存先のフォルダ）は残るので、不要なら手で削除してください。

### インストールせずに動かす

```sh
cargo run --release
```

## 保存先

保存先は「設定」の「保存先」で変えられます（既定は `~/Pictures/BingWallpaper`、日本語環境では `~/ピクチャ/BingWallpaper`）。設定は `~/.config/bing-wallpaper-manager/config.json` に、ウィンドウの大きさなどは同じフォルダの `state.json` に保存されます。
同じ写真は地域や配信日が違っても 1 枚だけ保存し、保存済みの画像は再ダウンロードしません。

```text
<保存先>/
  20261108_OHR.Foo_UHD.jpg              壁紙用の画像（UHD）
  .bing-wallpaper/meta/OHR.Foo.json     見出し・解説文・配信地域・色の割合などのメタデータ
  .bing-wallpaper/thumbs/OHR.Foo.jpg    サムネイル (640x360)
  .bing-wallpaper/previews/OHR.Foo.jpg  古い写真を大きく表示するための中サイズ (1920x1080)
  .bing-wallpaper/deleted.txt           削除した写真（取得し直さないための記録）
```

## コマンドラインで使う

```sh
bing-wallpaper-manager --apply                       # 今日の写真を壁紙にする
bing-wallpaper-manager --apply --mkt en-US           # 地域を指定
bing-wallpaper-manager --apply --if-new              # 前回と同じ写真なら何もしない
bing-wallpaper-manager --apply --random              # 保存した写真から 1 枚選ぶ
bing-wallpaper-manager --apply --random --favorites  # お気に入りから 1 枚選ぶ
```

`--mkt` を付けないときは、「自動更新」で選んだ地域（無ければ表示している最初の地域）を使います。ログイン直後などでネットワークがまだつながっていないときは、何度かやり直してから終了します。

### 毎日自動で更新する

**設定** → **自動更新** を使ってください（[自動更新](#自動更新) を参照）。systemd のユーザータイマーをアプリが作ります。動いているかは次で確かめられます。

```sh
systemctl --user list-timers bing-wallpaper-manager.timer
journalctl --user -u bing-wallpaper-manager.service
```

以前の版の手順で `bing-wallpaper.timer` を手で作っていた場合は、二重に動かないように止めてください: `systemctl --user disable --now bing-wallpaper.timer`

## 開発

### 見た目の確認（スクリーンショット）

仮想の Wayland ディスプレイ（sway の headless モード）でアプリを動かし、PNG を撮ります。
今のデスクトップ・設定・保存先・壁紙・systemd のタイマーには触れません（撮影用の設定と画像は `target/screenshot-fixture/` に置かれます）。

```sh
sudo apt install sway grim          # 初回のみ
cargo build --release
scripts/screenshot.sh target/shot.png            # 既定: 15 秒待って 1440x960 で撮影
BWM_THEME=dark BWM_LANG=de scripts/screenshot.sh target/dark.png 10 1280x1300
```

`BWM_THEME=light|dark` で配色を、`BWM_LANG=de` などで表示言語を固定できます（通常の起動でも使えます）。
`FLOAT=1280x840` でウィンドウを画面いっぱいに広げず浮かせて撮れます（角の丸みや窓のボタンの確認用）。`BG=<画像>` でその後ろのデスクトップの背景に画像を敷けます。

`ACTIONS` にクリック・右クリック・ドラッグ・スクロール・キー入力・文字入力を書くと、撮影前に操作できます。仮想マウスと仮想キーボードには `tools/vpointer`（初回に自動でビルド）を使います。

```sh
ACTIONS=$'key ctrl-f\ntype penguin\nsleep 3\nshot target/step1.png' \
  scripts/screenshot.sh target/step2.png 10 1280x1000
```

`docs/` の画像はこの方法でダークテーマで撮り（`README.md` 用は英語表示、`README.ja.md` 用の `*.ja.jpg` は日本語表示）、品質 85 の JPEG にしています。例:

```sh
BG=target/screenshot-fixture/store/20261112_OHR.AbrahamCanada_UHD.jpg FLOAT=1280x840 BWM_LANG=ja BWM_THEME=dark \
  ACTIONS=$'key enter\nsleep 2\nclick 720 250\nsleep 1.5' \
  scripts/screenshot.sh target/main.png 12 1440x980
```

### リリース用のパッケージ

`scripts/package-release.sh` がリリース版をビルドし、アプリ本体・アイコン・デスクトップ項目・ライセンス類（使用ライブラリのライセンス本文を含む）を入れた `target/dist/bing-wallpaper-manager-<バージョン>-x86_64-linux.tar.gz` と `.sha256` を作ります。バージョンは `Cargo.toml` から取ります。

### 翻訳

文言は `locales/<言語>.yml` にあります（`ja`、`en`、`zh-CN`、`zh-TW`、`ko`、`de`、`fr`、`es`、`it`、`pt-BR`、`ru`）。

- 値の差し込みは `%{count}` の形で書きます
- すべてのファイルに同じキーが必要です（`cargo test` で確認されます）。訳が無いキーは英語で表示されます
- 言語を追加するときは、`locales/` にファイルを足し、`src/i18n.rs` の `Language` に追加します

### 構成

| ファイル | 役割 |
| --- | --- |
| `src/main.rs` | ダッシュボード UI と CLI |
| `src/bing.rs` | Bing の HPImageArchive API、画像ダウンロード |
| `src/archive.rs` | 有志のアーカイブからの古い写真と解説文 |
| `src/library.rs` | 保存先への写真の保管と読み込み |
| `src/search.rs` | 言葉と色での検索 |
| `src/colors.rs` | 写真の色の割合の解析 |
| `src/settings.rs` | 設定ダイアログ |
| `src/info.rs` | 写真の情報のパネル |
| `src/schedule.rs` | 壁紙の自動更新（systemd のユーザータイマー） |
| `src/single.rs` | 二重起動の防止（もう一度起動すると、開いているウィンドウを前に出す） |
| `src/config.rs` | 設定ファイル |
| `src/theme.rs` | 配色（ライト／ダーク）とフォント |
| `src/i18n.rs` | 表示言語の切り替え・日付の書式（文言は `tr!("キー")` で参照） |
| `src/wallpaper.rs` | `gsettings` による GNOME の壁紙設定 |
| `locales/*.yml` | 翻訳ファイル（rust-i18n でビルド時に埋め込み） |
| `src/third_party.tsv` / `THIRD_PARTY_LICENSES.md` | 使用ライブラリの一覧（`scripts/gen-third-party.py` で生成） |
| `assets/icons/` | アプリのアイコン（SVG と、`scripts/render-icon.py` で作る PNG） |
| `assets/bing-wallpaper-manager.desktop` | デスクトップ項目（`scripts/install.sh` が入れる） |
| `scripts/rustc-stable` | cargo が使う rustc を rustup の stable から選ぶ（「ビルド」参照） |
| `scripts/screenshot.sh` | 仮想ディスプレイでのスクリーンショット撮影 |
| `tools/vpointer/` | 撮影時に操作を送る仮想マウス・キーボード（別プロジェクト） |

## ライセンス

[MIT License](LICENSE-MIT) と [Apache License 2.0](LICENSE-APACHE) のどちらかを選んで利用できます（MIT OR Apache-2.0）。使用しているライブラリの一覧は [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) にあります（依存を変えたら `python3 scripts/gen-third-party.py` で作り直す）。

写真の著作権は各撮影者・配信元に帰属します。このアプリは Microsoft とは関係ありません。
