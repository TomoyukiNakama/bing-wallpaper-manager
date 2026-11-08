#!/usr/bin/env bash
# GitHub のリリースに添付する、ビルド済みパッケージ（x86_64 Linux）を作る。
#
#   scripts/package-release.sh
#
# target/dist/ に次の 2 つができる（バージョンは Cargo.toml から取る）:
#   bing-wallpaper-manager-<version>-x86_64-linux.tar.gz
#   bing-wallpaper-manager-<version>-x86_64-linux.tar.gz.sha256
#
# パッケージの中身は ~/.local と同じ並び（bin/、share/applications/、share/icons/、share/doc/）で、
#   tar -xzf <パッケージ> -C ~/.local --strip-components=1
# で使える。share/doc/ には、使用しているライブラリのライセンス本文も入れる。
set -euo pipefail

cd "$(dirname "$0")/.."
APP=bing-wallpaper-manager
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
NAME=$APP-$VERSION-x86_64-linux
DIST=target/dist
DIR=$DIST/$NAME

cargo build --release
rm -rf "$DIR" "$DIST/$NAME.tar.gz" "$DIST/$NAME.tar.gz.sha256"
mkdir -p "$DIST"

install -Dm755 "target/release/$APP" "$DIR/bin/$APP"
strip "$DIR/bin/$APP"
install -Dm644 "assets/$APP.desktop" "$DIR/share/applications/$APP.desktop"
install -Dm644 "assets/icons/$APP.svg" "$DIR/share/icons/hicolor/scalable/apps/$APP.svg"
for png in assets/icons/hicolor/*/apps/$APP.png; do
    size=$(basename "$(dirname "$(dirname "$png")")")
    install -Dm644 "$png" "$DIR/share/icons/hicolor/$size/apps/$APP.png"
done
for f in README.md LICENSE-MIT LICENSE-APACHE THIRD_PARTY_LICENSES.md; do
    install -Dm644 "$f" "$DIR/share/doc/$APP/$f"
done
python3 -I scripts/gen-third-party.py --texts "$DIR/share/doc/$APP/THIRD_PARTY_LICENSE_TEXTS.txt"

(cd "$DIST" && tar -czf "$NAME.tar.gz" "$NAME" && sha256sum "$NAME.tar.gz" >"$NAME.tar.gz.sha256")
echo "作成しました: $DIST/$NAME.tar.gz"
cat "$DIST/$NAME.tar.gz.sha256"
