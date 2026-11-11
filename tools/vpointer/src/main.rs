//! 撮影スクリプト（scripts/screenshot.sh）用の仮想マウス。
//!
//! headless の sway には入力デバイスが無く、wlrctl はコマンドのたびに仮想マウスを
//! 作っては消すため、アプリがマウスを認識する前に消えてしまう。
//! このツールは起動中ずっと 1 つの仮想マウスを保ち、標準入力から操作を受け取る。
//!
//! 使い方: vpointer <幅> <高さ>
//!   move X Y      絶対座標へ移動
//!   click         左クリック
//!   rclick        右クリック
//!   press / release  左ボタンを押す／離す（ドラッグ用）
//!   scroll N      N 段スクロール（正で下、負で上）
//!   key NAME      キーを押して離す（left / right / up / down / enter / escape / f5 / ctrl-英字）
//!   type TEXT     英小文字・数字・空白・ハイフンを入力する

use std::io::{BufRead, Write};
use std::os::fd::AsFd;
use std::thread::sleep;
use std::time::{Duration, Instant};

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_pointer, wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;

struct State;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(State: ignore wl_seat::WlSeat);
delegate_noop!(State: ZwlrVirtualPointerManagerV1);
delegate_noop!(State: ZwlrVirtualPointerV1);
delegate_noop!(State: ZwpVirtualKeyboardManagerV1);
delegate_noop!(State: ZwpVirtualKeyboardV1);

/// 標準の US 配列。include はクライアント側の libxkbcommon が /usr/share/X11/xkb から解決する。
const KEYMAP: &str = r#"xkb_keymap {
  xkb_keycodes { include "evdev+aliases(qwerty)" };
  xkb_types { include "complete" };
  xkb_compat { include "complete" };
  xkb_symbols { include "pc+us+inet(evdev)" };
};
"#;

/// 文字 → evdev のキーコード（US 配列、英小文字・数字・空白・ハイフンのみ）。
fn char_keycode(c: char) -> Option<u32> {
    const LETTERS: [u32; 26] = [
        30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45, 21, 44,
    ];
    Some(match c {
        'a'..='z' => LETTERS[(c as u8 - b'a') as usize],
        '1'..='9' => 2 + (c as u8 - b'1') as u32,
        '0' => 11,
        ' ' => 57,
        '-' => 12,
        _ => return None,
    })
}

/// evdev のキーコード。
fn keycode(name: &str) -> Option<u32> {
    Some(match name {
        "escape" => 1,
        "enter" => 28,
        "f5" => 63,
        "up" => 103,
        "left" => 105,
        "right" => 106,
        "down" => 108,
        _ => return None,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<u32> = std::env::args().skip(1).map(|a| a.parse()).collect::<Result<_, _>>()?;
    let [width, height] = args[..] else {
        return Err("使い方: vpointer <幅> <高さ>".into());
    };

    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn)?;
    let qh = queue.handle();
    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=7, ())?;
    let manager: ZwlrVirtualPointerManagerV1 = globals.bind(&qh, 1..=2, ())?;
    let pointer = manager.create_virtual_pointer(Some(&seat), &qh, ());

    let keyboard_manager: ZwpVirtualKeyboardManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let keyboard = keyboard_manager.create_virtual_keyboard(&seat, &qh, ());
    let keymap_path = std::env::temp_dir().join(format!("vpointer-keymap-{}", std::process::id()));
    let mut keymap = std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&keymap_path)?;
    std::fs::remove_file(&keymap_path)?;
    keymap.write_all(KEYMAP.as_bytes())?;
    keymap.write_all(&[0])?;
    keyboard.keymap(1 /* XKB_V1 */, keymap.as_fd(), KEYMAP.len() as u32 + 1);
    queue.roundtrip(&mut State)?;
    println!("ready");

    let start = Instant::now();
    let time = || start.elapsed().as_millis() as u32;
    let mut state = State;

    for line in std::io::stdin().lock().lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts[..] {
            ["move", x, y] => {
                pointer.motion_absolute(time(), x.parse()?, y.parse()?, width, height);
                pointer.frame();
            }
            [cmd @ ("click" | "rclick")] => {
                let button = if cmd == "click" { BTN_LEFT } else { BTN_RIGHT };
                pointer.button(time(), button, wl_pointer::ButtonState::Pressed);
                pointer.frame();
                conn.flush()?;
                sleep(Duration::from_millis(60));
                pointer.button(time(), button, wl_pointer::ButtonState::Released);
                pointer.frame();
            }
            [cmd @ ("press" | "release")] => {
                let state = if cmd == "press" {
                    wl_pointer::ButtonState::Pressed
                } else {
                    wl_pointer::ButtonState::Released
                };
                pointer.button(time(), BTN_LEFT, state);
                pointer.frame();
            }
            ["scroll", n] => {
                let n: i32 = n.parse()?;
                for _ in 0..n.unsigned_abs() {
                    let dir = n.signum();
                    pointer.axis_source(wl_pointer::AxisSource::Wheel);
                    pointer.axis_discrete(
                        time(),
                        wl_pointer::Axis::VerticalScroll,
                        15.0 * dir as f64,
                        dir,
                    );
                    pointer.frame();
                    conn.flush()?;
                    sleep(Duration::from_millis(30));
                }
            }
            ["key", name] if name.starts_with("ctrl-") => {
                // ctrl-英字（ctrl-f、ctrl-d など）
                let Some(code) = name[5..].chars().next().and_then(char_keycode) else {
                    eprintln!("不明なキー: {name}");
                    continue;
                };
                const CTRL: u32 = 29;
                keyboard.key(time(), CTRL, 1);
                keyboard.modifiers(4, 0, 0, 0); // Control
                conn.flush()?;
                keyboard.key(time(), code, 1);
                conn.flush()?;
                sleep(Duration::from_millis(40));
                keyboard.key(time(), code, 0);
                keyboard.key(time(), CTRL, 0);
                keyboard.modifiers(0, 0, 0, 0);
            }
            ["type", ..] => {
                let text = line.trim_start().strip_prefix("type").unwrap_or("").trim_start();
                for c in text.chars() {
                    let Some(code) = char_keycode(c) else { continue };
                    keyboard.key(time(), code, 1);
                    conn.flush()?;
                    sleep(Duration::from_millis(25));
                    keyboard.key(time(), code, 0);
                    conn.flush()?;
                    sleep(Duration::from_millis(25));
                }
            }
            ["key", name] => {
                let Some(code) = keycode(name) else {
                    eprintln!("不明なキー: {name}");
                    continue;
                };
                keyboard.key(time(), code, 1);
                conn.flush()?;
                sleep(Duration::from_millis(40));
                keyboard.key(time(), code, 0);
            }
            [] => {}
            _ => eprintln!("不明なコマンド: {line}"),
        }
        queue.roundtrip(&mut state)?;
    }
    Ok(())
}
