//! 二重起動の防止。
//!
//! 最初に起動したアプリが `$XDG_RUNTIME_DIR/bing-wallpaper-manager.sock` で待ち受ける。
//! あとから起動したアプリは、そこへ知らせを送って終了し、最初のアプリがウィンドウを前に出す。

use std::io::Write as _;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

fn socket_path() -> Option<PathBuf> {
    Some(dirs::runtime_dir()?.join("bing-wallpaper-manager.sock"))
}

pub enum Instance {
    /// すでに起動しているアプリに知らせた。このプロセスは終了する
    AlreadyRunning,
    /// このプロセスが最初。あとから起動したアプリの知らせを受け取る
    /// （待ち受けられない環境では None。そのときは二重起動を防がない）
    First(Option<Receiver<()>>),
}

pub fn acquire() -> Instance {
    let Some(path) = socket_path() else { return Instance::First(None) };
    if let Ok(mut stream) = UnixStream::connect(&path) {
        let _ = stream.write_all(b"show");
        return Instance::AlreadyRunning;
    }
    // 前のアプリが異常終了して残ったソケットは、つながらないので消して作り直す
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else { return Instance::First(None) };
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            if stream.is_ok() && tx.send(()).is_err() {
                break;
            }
        }
    });
    Instance::First(Some(rx))
}
