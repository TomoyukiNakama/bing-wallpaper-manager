#!/usr/bin/env bash
# アプリ・アイコン・デスクトップ項目をユーザーの領域（既定 ~/.local）に入れる。
# GNOME のアプリ一覧に「Bing 壁紙」が出るようになり、ウィンドウにもアイコンが付く。
# 毎回 cargo build --release してから入れるので、git pull のあとに実行すれば更新になる。
#
# 使い方: scripts/install.sh            # ~/.local に入れる
#         PREFIX=/path scripts/install.sh
#         scripts/install.sh --uninstall
set -euo pipefail

cd "$(dirname "$0")/.."
PREFIX=${PREFIX:-$HOME/.local}
APP=bing-wallpaper-manager

if [[ ${1:-} == --uninstall ]]; then
    rm -f "$PREFIX/bin/$APP" "$PREFIX/share/applications/$APP.desktop" \
        "$PREFIX/share/icons/hicolor/scalable/apps/$APP.svg"
    rm -f "$PREFIX"/share/icons/hicolor/*/apps/$APP.png
    echo "削除しました: $PREFIX"
    exit 0
fi

# 毎回ビルドする（git pull 後に古いバイナリを入れないため。変更が無ければすぐ終わる）
if ! command -v cargo >/dev/null; then
    echo "cargo が見つかりません。README.ja.md の「Rust を入れる」の手順で Rust をインストールしてください。" >&2
    echo "cargo not found. Install Rust as described under \"Install Rust\" in README.md." >&2
    exit 1
fi
cargo build --release

install -Dm755 "target/release/$APP" "$PREFIX/bin/$APP"
install -Dm644 "assets/$APP.desktop" "$PREFIX/share/applications/$APP.desktop"
install -Dm644 "assets/icons/$APP.svg" "$PREFIX/share/icons/hicolor/scalable/apps/$APP.svg"
for png in assets/icons/hicolor/*/apps/$APP.png; do
    size=$(basename "$(dirname "$(dirname "$png")")")
    install -Dm644 "$png" "$PREFIX/share/icons/hicolor/$size/apps/$APP.png"
done

# アイコンとアプリ一覧のキャッシュを更新（コマンドが無ければ省略）
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$PREFIX/share/icons/hicolor" || true
command -v update-desktop-database >/dev/null && update-desktop-database -q "$PREFIX/share/applications" || true

echo "インストールしました: $PREFIX/bin/$APP"
