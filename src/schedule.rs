//! 壁紙の自動更新。systemd のユーザータイマーで、このアプリの `--apply` を定期的に実行する。
//!
//! ```text
//! ~/.config/systemd/user/bing-wallpaper-manager.service   --apply を 1 回実行する
//! ~/.config/systemd/user/bing-wallpaper-manager.timer     実行する時刻
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, Result, bail};

use crate::config::{AutoMode, AutoUpdate};

const UNIT: &str = "bing-wallpaper-manager";

fn unit_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("systemd").join("user")
}

/// `--apply` に付ける引数。
fn apply_args(auto: &AutoUpdate, market: &str) -> Option<String> {
    if !auto.enabled {
        return None;
    }
    Some(match auto.mode {
        AutoMode::Today => format!("--apply --if-new --mkt {market}"),
        AutoMode::Favorites => "--apply --random --favorites".to_string(),
        AutoMode::Random => "--apply --random".to_string(),
    })
}

/// サービスとタイマーの内容。自動更新しない設定なら None。
fn unit_texts(exe: &Path, auto: &AutoUpdate, market: &str) -> Option<(String, String)> {
    let args = apply_args(auto, market)?;
    // 空白などを含むパスでも 1 つの引数になるように引用符で囲む（% は systemd の指定子なので重ねる）
    let exe = exe.display().to_string().replace('%', "%%").replace('\\', "\\\\").replace('"', "\\\"");
    let service = format!(
        "[Unit]\nDescription=Bing Wallpaper Manager: update the wallpaper\n\n\
         [Service]\nType=oneshot\nExecStart=\"{exe}\" {args}\n"
    );
    // 今日の写真: ログイン直後と 1 時間ごとに確かめ、新しい写真が配信されていたときだけ壁紙を変える
    let schedule = match auto.mode {
        AutoMode::Today => "OnStartupSec=1min\nOnCalendar=hourly\n".to_string(),
        _ => format!("OnCalendar={}\n", auto.interval.on_calendar()),
    };
    let timer = format!(
        "[Unit]\nDescription=Bing Wallpaper Manager: update the wallpaper\n\n\
         [Timer]\n{schedule}Persistent=true\n\n\
         [Install]\nWantedBy=timers.target\n"
    );
    Some((service, timer))
}

fn systemctl(args: &[&str]) -> Result<()> {
    // 撮影やテストでは、実際の systemd には触れない
    if std::env::var_os("BWM_NO_SYSTEMD").is_some() {
        return Ok(());
    }
    let output = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .context(tr!("error.systemctl_missing"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!(tr!("error.systemctl_failed", command = args.join(" "), detail = detail));
    }
    Ok(())
}

/// 設定に合わせてタイマーを作り直す（自動更新しない設定なら止めて消す）。
pub fn apply(auto: &AutoUpdate, market: &str) -> Result<()> {
    let dir = unit_dir();
    let service = dir.join(format!("{UNIT}.service"));
    let timer = dir.join(format!("{UNIT}.timer"));
    let timer_unit = format!("{UNIT}.timer");
    let exe = std::env::current_exe().context(tr!("error.current_exe"))?;
    match unit_texts(&exe, auto, market) {
        Some((service_text, timer_text)) => {
            fs::create_dir_all(&dir).with_context(|| tr!("error.create_dir", path = dir.display()))?;
            fs::write(&service, service_text).with_context(|| tr!("error.write", path = service.display()))?;
            fs::write(&timer, timer_text).with_context(|| tr!("error.write", path = timer.display()))?;
            systemctl(&["daemon-reload"])?;
            systemctl(&["enable", &timer_unit])?;
            // 動いているタイマーにも新しい時刻を反映する
            systemctl(&["restart", &timer_unit])?;
        }
        None => {
            if timer.exists() {
                // 止められなくても（systemd が無いなど）、ファイルは消す
                let stopped = systemctl(&["disable", "--now", &timer_unit]);
                let _ = fs::remove_file(&timer);
                let _ = fs::remove_file(&service);
                systemctl(&["daemon-reload"])?;
                stopped?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Interval;

    #[test]
    fn today_checks_hourly_and_only_applies_new_photos() {
        let auto = AutoUpdate { enabled: true, mode: AutoMode::Today, market: None, interval: Interval::Daily };
        let (service, timer) = unit_texts(Path::new("/opt/my apps/bwm"), &auto, "en-GB").unwrap();
        assert!(service.contains("ExecStart=\"/opt/my apps/bwm\" --apply --if-new --mkt en-GB\n"));
        assert!(timer.contains("OnStartupSec=1min\nOnCalendar=hourly\nPersistent=true\n"));
    }

    #[test]
    fn favorites_rotate_at_the_interval() {
        let auto = AutoUpdate { enabled: true, mode: AutoMode::Favorites, market: None, interval: Interval::Hours3 };
        let (service, timer) = unit_texts(Path::new("/usr/bin/bwm"), &auto, "ja-JP").unwrap();
        assert!(service.contains("ExecStart=\"/usr/bin/bwm\" --apply --random --favorites\n"));
        assert!(timer.contains("[Timer]\nOnCalendar=0/3:00\nPersistent=true\n"));
        assert!(unit_texts(Path::new("/usr/bin/bwm"), &AutoUpdate::default(), "ja-JP").is_none());
    }
}
