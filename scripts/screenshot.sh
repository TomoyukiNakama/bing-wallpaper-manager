#!/usr/bin/env bash
# 仮想の Wayland ディスプレイ（sway headless）でアプリを起動し、スクリーンショットを撮る。
# 今のデスクトップ・設定・保存先・壁紙には触れない（gsettings はメモリ上の仮の保存先を使う）。
#
# 使い方: scripts/screenshot.sh [出力PNG] [待機秒数] [幅x高さ]
#   環境変数 BWM_THEME=light|dark で配色を固定できる
#   環境変数 ACTIONS に操作を 1 行ずつ書くと、撮影前に順に実行する（座標は論理ピクセル）:
#     click X Y        X,Y を左クリック
#     rclick X Y       X,Y を右クリック
#     scroll down|up N X Y  X,Y の位置で N 段スクロール
#     key NAME         キーを押す（left / right / up / down / enter / escape / f5 / ctrl-英字）
#     type TEXT        英小文字・数字・空白・ハイフンを入力する
#     sleep 秒
#     shot PATH        途中経過を撮影
#   必要なもの: sway, grim（ACTIONS では tools/vpointer を自動でビルドして使う）
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
    GSETTINGS_BACKEND=memory \
    VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
    "$BIN" >"$FIXTURE/app.log" 2>&1 &
APP_PID=$!

sleep "$WAIT"
if ! kill -0 "$APP_PID" 2>/dev/null; then
    echo "アプリが終了しました。ログ: $FIXTURE/app.log" >&2
    tail -20 "$FIXTURE/app.log" >&2
    exit 1
fi

shot() {
    mkdir -p "$(dirname "$1")"
    XDG_RUNTIME_DIR="$RUNTIME" WAYLAND_DISPLAY="$SOCK" grim "$1"
}
# headless の sway には入力デバイスが無いので、tools/vpointer の仮想マウス・キーボードで操作する
# （wlrctl はコマンドごとにマウスを作り直すため、アプリに入力が届かない）
if [[ -n ${ACTIONS:-} ]]; then
    VPOINTER=tools/vpointer/target/release/vpointer
    if [[ ! -x $VPOINTER ]]; then
        (cd tools/vpointer && cargo build --release -q)
    fi
    mkfifo "$RUNTIME/vpointer"
    XDG_RUNTIME_DIR="$RUNTIME" WAYLAND_DISPLAY="$SOCK" \
        "$VPOINTER" "${SIZE%x*}" "${SIZE#*x}" <"$RUNTIME/vpointer" >"$RUNTIME/vpointer.log" 2>&1 &
    exec 3>"$RUNTIME/vpointer"
    sleep 0.5 # アプリがマウスを認識するのを待つ

    while read -r cmd a b c d; do
        case $cmd in
            "" | "#"*) ;;
            click | rclick)
                echo "move $a $b" >&3
                sleep 0.2
                echo "$cmd" >&3
                sleep 0.6 ;;
            scroll)
                echo "move $c $d" >&3
                sleep 0.2
                echo "scroll $([[ $a == up ]] && echo "-$b" || echo "$b")" >&3
                sleep 0.6 ;;
            key)
                echo "key $a" >&3
                sleep 0.5 ;;
            type)
                echo "type $a${b:+ $b}${c:+ $c}${d:+ $d}" >&3
                sleep 1 ;;
            sleep) sleep "$a" ;;
            shot) shot "$a"; echo "保存しました: $a" ;;
            *) echo "不明な操作: $cmd" >&2; exit 1 ;;
        esac
    done <<< "$ACTIONS"
    exec 3>&-
fi

shot "$OUT"
echo "保存しました: $OUT"
