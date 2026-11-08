#!/usr/bin/env bash
# 仮想の Wayland ディスプレイ（sway headless）でアプリを起動し、スクリーンショットを撮る。
# 今のデスクトップ・設定・保存先には触れない。
#
# 使い方: scripts/screenshot.sh [出力PNG] [待機秒数] [幅x高さ]
#   環境変数 BWM_THEME=light|dark で配色を固定できる
#   必要なもの: sway, grim
set -euo pipefail

cd "$(dirname "$0")/.."
OUT=${1:-target/screenshot.png}
WAIT=${2:-15}
SIZE=${3:-1440x960}
BIN=${BIN:-target/release/bing-wallpaper-manager}

# 撮影用の設定と保存先（画像は再利用するので target/ 以下に残す）
FIXTURE=$PWD/target/screenshot-fixture
mkdir -p "$FIXTURE/config/bing-wallpaper-manager" "$FIXTURE/store"
printf '{"save_dir": "%s"}\n' "$FIXTURE/store" > "$FIXTURE/config/bing-wallpaper-manager/config.json"

RUNTIME=$(mktemp -d)
chmod 700 "$RUNTIME"
SWAY_PID=""
APP_PID=""
cleanup() {
    [[ -n $APP_PID ]] && kill "$APP_PID" 2>/dev/null || true
    [[ -n $SWAY_PID ]] && kill "$SWAY_PID" 2>/dev/null || true
    wait 2>/dev/null || true
    rm -rf "$RUNTIME"
}
trap cleanup EXIT

cat > "$RUNTIME/sway.conf" <<EOF
output HEADLESS-1 resolution $SIZE position 0 0
default_border none
default_floating_border none
EOF

env -u DISPLAY -u WAYLAND_DISPLAY \
    XDG_RUNTIME_DIR="$RUNTIME" \
    WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
    sway -c "$RUNTIME/sway.conf" >"$RUNTIME/sway.log" 2>&1 &
SWAY_PID=$!

for _ in $(seq 50); do
    SOCK=$(cd "$RUNTIME" && ls wayland-* 2>/dev/null | grep -v lock | head -1 || true)
    [[ -n $SOCK ]] && break
    sleep 0.1
done
if [[ -z ${SOCK:-} ]]; then
    echo "sway を起動できませんでした:" >&2
    cat "$RUNTIME/sway.log" >&2
    exit 1
fi

env -u DISPLAY \
    XDG_RUNTIME_DIR="$RUNTIME" \
    WAYLAND_DISPLAY="$SOCK" \
    XDG_CONFIG_HOME="$FIXTURE/config" \
    VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
    "$BIN" >"$FIXTURE/app.log" 2>&1 &
APP_PID=$!

sleep "$WAIT"
if ! kill -0 "$APP_PID" 2>/dev/null; then
    echo "アプリが終了しました。ログ: $FIXTURE/app.log" >&2
    tail -20 "$FIXTURE/app.log" >&2
    exit 1
fi

mkdir -p "$(dirname "$OUT")"
XDG_RUNTIME_DIR="$RUNTIME" WAYLAND_DISPLAY="$SOCK" grim "$OUT"
echo "保存しました: $OUT"
