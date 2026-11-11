# CLAUDE.md

Ubuntu 26.04（GNOME）向けの Bing 壁紙アプリ。Rust + gpui-kit。UI・メッセージは日本語。

## ビルド環境（必ず守る）

- ユーザーは **rustup の `cargo`（`~/.cargo/bin/cargo`、stable）** で `cargo run` する。
  `~/.profile` と `~/.bashrc` で `~/.cargo/env` を読み込み、PATH に `~/.cargo/bin` が入っている。
  Ubuntu 標準の cargo は `/usr/bin/cargo-1.93` だけで、`/usr/bin/cargo` は無い。
- gpui-kit が依存する gpui-pre は rustc 1.93 ではビルドできない（`std::hint::cold_path`）。
  ディストリビューションの cargo でもビルドできるよう、`.cargo/config.toml` で rustc を `scripts/rustc-stable` にしている。
  このスクリプトが rustup の stable の rustc 本体（`$RUSTUP_HOME/toolchains/stable-*/bin/rustc`）を探して実行する
  （無ければ `rustup which`、PATH の rustc の順）。rustup の cargo でも、この設定を通して同じ rustc が使われる。
  - **rustup のプロキシ（`~/.cargo/bin/rustc`）を直接使うように変えない。**
    プロキシは依存クレートに同梱された `rust-toolchain.toml`（1.83 指定など）でクレートごとにコンパイラを切り替え、
    `E0514 compiled by an incompatible version of rustc` で壊れる。
  - 個人のパス（`/home/...`）を設定ファイルに書かない（リポジトリは public）。
  - `rust-toolchain.toml`（stable）は rustup 版 cargo のためのもの。
  - `cargo-1.93` は `RUSTC` 環境変数で rustc 1.93 を指定するため `.cargo/config.toml` の設定より優先され、ビルドできない。使わない。
- 確認には **ユーザーと同じ cargo（rustup の cargo）を使う**。シェル設定に左右されないよう、PATH を明示して実行する。
  ```sh
  env -i HOME=$HOME PATH=$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin bash -c 'cd /mnt/ssd2/git/bing-wallpaper-manager && cargo build && cargo test'
  ```
  （cargo の種類が変わるとキャッシュが合わず全体を再ビルドするので、普段からこの cargo に揃える）
  以下の `cargo build --release` や `timeout 12 cargo run` も同じ PATH で実行する。
- 依存の追加・更新をしたら、上のコマンドでビルドが通ることを必ず確認する。
- `E0514` や `cold_path` のエラーが出たら、古い成果物が原因のことがある。`rm -rf target/debug`（または `target/release`）してから再ビルドする。

## 変更後の確認手順（完了報告の前に必ず行う）

1. 上記の素の cargo で `cargo build` と `cargo test` が通る。
2. `timeout 12 cargo run` が終了コード 124（＝起動したまま）になる。すぐ終了したらエラーを確認する。
3. UI を変えたら、仮想ディスプレイで撮影して PNG を実際に見る（ライト・ダーク両方）。
   ```sh
   cargo build --release
   BWM_THEME=light scripts/screenshot.sh target/shot-light.png 10 1440x1500
   BWM_THEME=dark  scripts/screenshot.sh target/shot-dark.png  10 1280x1000
   ```
   sway の headless モードと grim を使い、今のデスクトップ・設定・保存先には触れない。
   クリックやスクロールが関わる変更は、`ACTIONS` で実際に操作して撮る（静止画だけで済ませない）。
   ```sh
   ACTIONS=$'scroll down 8 700 800\nclick 880 830\nshot target/step1.png' scripts/screenshot.sh target/step2.png 10 1280x1000
   ```
   使える操作は `click X Y` / `rclick X Y` / `drag X1 Y1 X2 Y2` / `scroll down|up N X Y` / `key left|right|enter|escape|f5|ctrl-f` /
   `type 英小文字・数字・空白` / `sleep 秒` / `shot PATH`。
   撮影中の gsettings はメモリ上の仮の保存先（`GSETTINGS_BACKEND=memory`）なので、「壁紙にする」を押しても実際の壁紙は変わらない。
   写真が多いときの動作（スクロールの重さなど）は、`STORE=<写真が数百枚ある保存先のコピー>` で保存先を差し替えて確かめる
   （撮影はソフトウェア描画なので、実機より遅い）。
   入力は `tools/vpointer`（常駐する仮想マウス・キーボード）で送る。wlrctl はコマンドごとにマウスを作り直すため、
   アプリに入力が届かないので使わない。座標は先に撮った PNG で確かめてから指定する。
4. 確認できなかったこと（クリック操作、アニメーションなど）は、そのまま報告に書く。

## 実行環境の注意

- このマシンでは Bash のサンドボックス（bubblewrap）が起動できないため、コマンドはサンドボックス外で実行する。
- ユーザーの壁紙（gsettings）を変える確認をしたら、元の値に戻す。
- `~/.bashrc` などシェル設定の書き換えは自動では許可されない。必要ならユーザーに頼む。

## 構成とデザイン方針

- `src/bing.rs` Bing API・ダウンロード / `src/library.rs` 保存先への保管 / `src/config.rs` 設定 /
  `src/theme.rs` 配色とフォント / `src/wallpaper.rs` gsettings / `src/main.rs` UI と CLI（`--apply`）
- 保存先には UHD 画像を置き、`.bing-wallpaper/meta`・`.bing-wallpaper/thumbs` にメタデータとサムネイルを置く。
  同じ写真は地域や配信日が違っても `OHR.名前` をキーに 1 枚だけ保存する。
- ウィンドウは枠をアプリが描き（`WindowDecorations::Client`）、角を丸くする（`theme::WINDOW_RADIUS`）。gpui の切り抜きは四角いので、
  窓の角に接する要素（大きな写真の画像・下地、タイトルバー、フッター）はそれぞれ `window_corner_radii` で角を丸める。
  タイトルバーは gpui-kit の `TitleBar` を使わず、大きな写真に重ねて、マウスが写真に乗っている間だけ出す（`render_window_bar`）。
  ウィンドウの移動は大きな写真全体で受け付けるので、写真の上に置くボタンは `on_mouse_down` で伝播を止める。
  角・影・窓のボタンは `FLOAT=1100x820 scripts/screenshot.sh ...` でウィンドウを浮かせて撮って確かめる。
- デザイン（frontend-design skill で決めたもの）: 写真が主役。上部に選択中の写真を全幅表示して固定し（一覧だけスクロール）、
  その下に「今日の写真」（写真ごとに配信地域をまとめる）と「過去の写真」（日付ごとのまとまりを横に流す）。
  配色は `theme.rs` のパレット（霧と湖の色）以外を直書きしない。同じカードの羅列、影、
  中黒（・）区切りのメタ情報、全カードへのホバー演出は使わない。
- UI の部品選びや操作の作法は gpui-kit の公式 skill（`gpui-kit`、`gpui-kit-design-guides`）のガイドに従う。
  例: 主要な操作はキーボード（Actions）でも使えるようにする、写真への操作は右クリックメニューと「…」ボタンの両方から出す、
  設定はフッターではなく設定パネル（Sheet）に置く、押せない「状態」は無効化ボタンではなく表示として出す。
- Wayland では gpui の `primary_display()` が常に None。モニター情報は `window.display(cx)` から取る。
- 画面の文言はコードに直接書かず、`locales/*.yml`（11 言語）にキーを追加して `tr!("キー")` / `tr!("キー", count = n)` で参照する。
  新しいキーは全言語のファイルに追加する（`cargo test` の `every_language_has_every_key` で確認される）。差し込みは `%{name}`。
  各言語の見た目は `BWM_LANG=de scripts/screenshot.sh ...` のように言語を固定して撮影して確認する。
- 設定ダイアログ（SettingsView）は Dashboard の更新中に作られる。`SettingsView::new` の中で Dashboard を read しない（パニックする）。
- 依存を追加・更新したら `python3 scripts/gen-third-party.py` で使用ライブラリの一覧を作り直す。
- アーカイブ（bing.npanuhin.me）は非公式。使えないときも公式 API の範囲だけで動くようにする。
- README は英語版（`README.md`、GitHub の既定）と日本語版（`README.ja.md`）の 2 つ。機能を変えたら両方を同じ内容に更新する。
  説明用の画像は `docs/` にあり、英語版は英語表示、日本語版は日本語表示（`*.ja.jpg`）で撮る（`BWM_LANG`、ダークテーマ）。
  サイズを抑えるため、撮った PNG は品質 85 の JPEG にして置く。
  README 用は `FLOAT=1280x840 BG=<表示中の写真の UHD 画像>` で、壁紙の上に浮かせたウィンドウとして 1440x980 で撮る
  （手順は README の「Development」→「Screenshots」に例がある）。
- アイコンは `assets/icons/bing-wallpaper-manager.svg` が元。変えたら `python3 scripts/render-icon.py` で PNG を作り直す。
  ウィンドウの app_id と `.desktop` のファイル名（`bing-wallpaper-manager`）は一致させる（GNOME がアイコンを結び付ける）。
- 一覧は gpui の `list`（仮想リスト）で、見えている行だけを描く。写真を日付ごとにまとめて、ウィンドウ幅に収まる行（`ListItem`）に詰める。
  写真が数百枚になるので、描画中に呼ぶ処理でファイルの有無を確かめたり（`is_file`）、全件を線形に探したりしない（`stored_index` を使う）。
- 検索（`src/search.rs`）は保存済みの写真のメタデータだけを対象にする。解説文は最初の検索時にアーカイブから補い、
  色の割合（`src/colors.rs`）は取得・読み込みのあとにサムネイルから裏で解析する。

## リリースの手順

1. `Cargo.toml` の version を上げる（「このアプリについて」に出る）。
2. `scripts/package-release.sh` で `target/dist/` にパッケージと `.sha256` を作る（使用ライブラリのライセンス本文も同梱される）。
3. パッケージを一時フォルダに展開し、`BIN=<展開先>/bin/bing-wallpaper-manager scripts/screenshot.sh ...` で起動を確認する。
4. コミット・push してから `gh release create v<version> --target main --notes-file ... <tar.gz> <tar.gz.sha256>`。
   リリースノートは英語で書き、日本語の説明も付ける。v0.1.0 との差分は `git fetch --tags` してから `git log v<前>..HEAD`。
5. 公開された URL からダウンロードし、`sha256sum -c` が通ることを確かめる（作成直後は 504 になることがある）。
