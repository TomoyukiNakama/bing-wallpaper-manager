#!/usr/bin/env bash
# アプリ・アイコン・デスクトップ項目をユーザーの領域（既定 ~/.local）に入れる。
# GNOME のアプリ一覧に「Bing 壁紙」が出るようになり、ウィンドウにもアイコンが付く。
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

if [[ ! -x target/release/$APP ]]; then
    cargo build --release
fi

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
