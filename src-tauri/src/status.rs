//! 動いているタイマーとポモドーロの「見える化」：メニューバーのネコの隣に、残り時間
//! （🍅 12:30 など）を出す。タイマーとポモドーロそれぞれの設定で、出すかを切り替えられる

use std::{sync::Arc, thread, time::Duration};

use tauri::AppHandle;

use crate::{pomodoro::Phase, reminder::now_ms, settings::Settings, Shared};

const INTERVAL: Duration = Duration::from_secs(1);

/// 残りミリ秒を「m:ss」（1 時間以上は「h:mm:ss」）にする。過ぎていれば 0:00
pub fn format_remaining(ms: i64) -> String {
    let total = (ms.max(0) + 999) / 1000;
    let (h, m, s) = (total / 3600, total % 3600 / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// いまの設定から、メニューバーに出す文字を決める（出さないときは空）。
/// ポモドーロとタイマーが両方動いていれば、両方を並べる。タイマーは、いちばん早く終わるものだけを出す
pub fn menubar_title(settings: &Settings, now: i64) -> String {
    let mut parts = Vec::new();
    if let (true, Some(p)) = (settings.menubar_pomodoro, settings.pomodoro) {
        let icon = if p.phase == Phase::Work {
            "🍅"
        } else {
            "☕"
        };
        parts.push(format!("{icon} {}", format_remaining(p.ends_at - now)));
    }
    let next = settings
        .reminders
        .iter()
        .filter(|r| r.timer)
        .map(|r| r.at)
        .min();
    if let (true, Some(next)) = (settings.menubar_timer, next) {
        parts.push(format!("⏱ {}", format_remaining(next - now)));
    }
    parts.join("  ")
}

pub fn spawn(app: AppHandle, shared: Arc<Shared>) {
    thread::spawn(move || {
        let mut last: Option<String> = None;
        loop {
            thread::sleep(INTERVAL);
            // 画面を誰も見ていない間は、メニューバーの文字を更新しない
            if shared.paused() {
                continue;
            }
            let title = menubar_title(&shared.store.get(), now_ms());
            if last.as_deref() == Some(title.as_str()) {
                continue;
            }
            if let Some(tray) = app.tray_by_id("main-tray") {
                let _ = tray.set_title((!title.is_empty()).then_some(title.as_str()));
            }
            last = Some(title);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pomodoro::Pomodoro,
        reminder::{Reminder, Repeat},
    };

    const NOW: i64 = 1_000_000;

    fn timer(id: u64, at: i64) -> Reminder {
        Reminder {
            timer: true,
            ..Reminder::new(id, "t".into(), at, Repeat::Once, 0)
        }
    }

    fn work(ends_in: i64) -> Option<Pomodoro> {
        Some(Pomodoro {
            phase: Phase::Work,
            ends_at: NOW + ends_in,
            cycle: 1,
        })
    }

    #[test]
    fn remaining_time_format() {
        assert_eq!(format_remaining(0), "0:00");
        assert_eq!(format_remaining(-5000), "0:00");
        assert_eq!(format_remaining(1), "0:01");
        assert_eq!(format_remaining(61_000), "1:01");
        assert_eq!(format_remaining(25 * 60_000), "25:00");
        assert_eq!(format_remaining(3_723_000), "1:02:03");
    }

    #[test]
    fn nothing_running_shows_nothing() {
        assert_eq!(menubar_title(&Settings::default(), NOW), "");
    }

    #[test]
    fn pomodoro_work_and_rest() {
        let settings = Settings {
            pomodoro: work(12 * 60_000 + 30_000),
            ..Settings::default()
        };
        assert_eq!(menubar_title(&settings, NOW), "🍅 12:30");

        let rest = Settings {
            pomodoro: Some(Pomodoro {
                phase: Phase::Longbreak,
                ends_at: NOW + 60_000,
                cycle: 4,
            }),
            ..Settings::default()
        };
        assert_eq!(menubar_title(&rest, NOW), "☕ 1:00");
    }

    #[test]
    fn shows_only_the_earliest_timer() {
        let settings = Settings {
            reminders: vec![timer(1, NOW + 300_000), timer(2, NOW + 161_000)],
            ..Settings::default()
        };
        assert_eq!(menubar_title(&settings, NOW), "⏱ 2:41");
    }

    #[test]
    fn plain_reminders_are_not_shown() {
        let settings = Settings {
            reminders: vec![Reminder::new(1, "r".into(), NOW + 1000, Repeat::Once, 0)],
            ..Settings::default()
        };
        assert_eq!(menubar_title(&settings, NOW), "");
    }

    #[test]
    fn both_running_show_both() {
        let settings = Settings {
            pomodoro: work(600_000),
            reminders: vec![timer(1, NOW + 180_000)],
            ..Settings::default()
        };
        assert_eq!(menubar_title(&settings, NOW), "🍅 10:00  ⏱ 3:00");
    }

    #[test]
    fn each_can_be_turned_off_separately() {
        let both = Settings {
            pomodoro: work(600_000),
            reminders: vec![timer(1, NOW + 180_000)],
            ..Settings::default()
        };
        let no_timer = Settings {
            menubar_timer: false,
            ..both.clone()
        };
        assert_eq!(menubar_title(&no_timer, NOW), "🍅 10:00");
        let no_pomodoro = Settings {
            menubar_pomodoro: false,
            ..both.clone()
        };
        assert_eq!(menubar_title(&no_pomodoro, NOW), "⏱ 3:00");
        let neither = Settings {
            menubar_timer: false,
            menubar_pomodoro: false,
            ..both
        };
        assert_eq!(menubar_title(&neither, NOW), "");
    }
}
