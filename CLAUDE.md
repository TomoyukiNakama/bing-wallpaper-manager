# CLAUDE.md

Ubuntu 26.04（GNOME）向けの Bing 壁紙アプリ。Rust + gpui-kit。UI・メッセージは日本語。

## ビルド環境（必ず守る）

- ユーザーは **素の `cargo`（`/usr/bin/cargo`、Ubuntu 標準 1.93）** で `cargo run` する。PATH に `~/.cargo/bin` は入っていない。
- gpui-kit が依存する gpui-pre は rustc 1.93 ではビルドできない（`std::hint::cold_path`）。そのため
  `.cargo/config.toml` で rustc を rustup の stable 本体
  （`~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc`）に固定している。
  - **この設定を消したり、`~/.cargo/bin/rustc`（rustup のプロキシ）に変えたりしない。**
    プロキシにすると、依存クレートに同梱された `rust-toolchain.toml`（1.83 指定など）で
    クレートごとにコンパイラが切り替わり、`E0514 compiled by an incompatible version of rustc` で壊れる。
  - `rust-toolchain.toml`（stable）は rustup 版 cargo を使う場合のためのもの。
- 確認には **ユーザーと同じ cargo を使う**。PATH に `~/.cargo/bin` を足した状態で確認して「動く」と報告しない。
  ```sh
  env -i HOME=$HOME PATH=/usr/local/bin:/usr/bin:/bin bash -c 'cd /mnt/ssd2/git/bing-wallpaper-manager && cargo build && cargo test'
  ```
  （cargo の種類が変わるとキャッシュが合わず全体を再ビルドするので、普段からこの cargo に揃える）
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
- デザイン（frontend-design skill で決めたもの）: 写真が主役。上部に選択中の写真を全幅表示し、
  その下に「今日の写真」（写真ごとに配信地域をまとめる）と「過去の写真」（日付ごとのまとまりを横に流す）。
  配色は `theme.rs` のパレット（霧と湖の色）以外を直書きしない。同じカードの羅列、影、
  中黒（・）区切りのメタ情報、全カードへのホバー演出は使わない。
