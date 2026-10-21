//! リマインダー：保存する内容、時刻になったかの判定、登録・一覧・削除の IPC、設定用ウィンドウ

use std::{
    sync::{atomic::Ordering, Arc},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::{about, Shared, MAIN_WINDOW};

pub const WINDOW: &str = "reminders";
/// 時刻になったかを調べる間隔
const CHECK_INTERVAL: Duration = Duration::from_secs(5);
/// ふきだしに収まる文字数
pub const MAX_TEXT_CHARS: usize = 30;
/// 登録できる件数
const MAX_REMINDERS: usize = 100;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    Once,
    Daily,
    Weekly,
}

impl Repeat {
    fn period_ms(self) -> Option<i64> {
        match self {
            Repeat::Once => None,
            Repeat::Daily => Some(DAY_MS),
            Repeat::Weekly => Some(7 * DAY_MS),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reminder {
    pub id: u64,
    pub text: String,
    /// 次に知らせる時刻（UNIX エポックからのミリ秒）
    pub at: i64,
    pub repeat: Repeat,
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 時刻になったものを取り出す。1 回きりのものは消し、くり返すものは次の時刻へ進める。
/// アプリを止めていた間に何回分かが過ぎていても、知らせるのは 1 回だけ
pub fn take_due(list: &mut Vec<Reminder>, now: i64) -> Vec<Reminder> {
    let mut fired = Vec::new();
    list.retain_mut(|r| {
        if r.at > now {
            return true;
        }
        fired.push(r.clone());
        match r.repeat.period_ms() {
            Some(period) => {
                r.at += ((now - r.at) / period + 1) * period;
                true
            }
            None => false,
        }
    });
    fired.sort_by_key(|r| r.at);
    fired
}

fn sorted(mut list: Vec<Reminder>) -> Vec<Reminder> {
    list.sort_by_key(|r| (r.at, r.id));
    list
}

#[tauri::command]
pub fn list_reminders(shared: State<Arc<Shared>>) -> Vec<Reminder> {
    sorted(shared.store.get().reminders)
}

#[tauri::command]
pub fn add_reminder(
    shared: State<Arc<Shared>>,
    text: String,
    at: i64,
    repeat: Repeat,
) -> Result<Vec<Reminder>, String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("内容を入力してください".into());
    }
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(format!("内容は {MAX_TEXT_CHARS} 文字以内にしてください"));
    }
    if repeat == Repeat::Once && at <= now_ms() {
        return Err("未来の日時を指定してください".into());
    }
    let mut result = Err("リマインダーが多すぎます".to_string());
    shared.store.update(|s| {
        if s.reminders.len() >= MAX_REMINDERS {
            return;
        }
        let id = s.reminders.iter().map(|r| r.id).max().unwrap_or(0) + 1;
        s.reminders.push(Reminder {
            id,
            text,
            at,
            repeat,
        });
        result = Ok(sorted(s.reminders.clone()));
    });
    result
}

#[tauri::command]
pub fn delete_reminder(shared: State<Arc<Shared>>, id: u64) -> Vec<Reminder> {
    shared.store.update(|s| s.reminders.retain(|r| r.id != id));
    sorted(shared.store.get().reminders)
}

/// 時刻になったリマインダーをマスコットのふきだしへ送る。
/// マスコットを非表示にしている間は送らず、表示に戻ったときに知らせる
pub fn spawn_scheduler(app: AppHandle, shared: Arc<Shared>) {
    thread::spawn(move || loop {
        thread::sleep(CHECK_INTERVAL);
        if !shared.visible.load(Ordering::Relaxed) {
            continue;
        }
        let now = now_ms();
        if !shared.store.get().reminders.iter().any(|r| r.at <= now) {
            continue;
        }
        let mut fired = Vec::new();
        shared
            .store
            .update(|s| fired = take_due(&mut s.reminders, now));
        let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
            continue;
        };
        for r in fired {
            let _ = window.emit("mascot-reminder", r.text);
        }
    });
}

/// リマインダーの設定ウィンドウを開く（開いていれば前面に出す）。メインスレッドから呼ぶこと
pub fn open_window(app: &AppHandle) {
    about::activate_app();
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let window = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::App("reminders.html".into()))
        .title("リマインダー")
        .inner_size(400.0, 560.0)
        .min_inner_size(340.0, 400.0)
        .center()
        .always_on_top(true)
        .build();
    if let Ok(window) = window {
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(id: u64, at: i64, repeat: Repeat) -> Reminder {
        Reminder {
            id,
            text: format!("r{id}"),
            at,
            repeat,
        }
    }

    #[test]
    fn once_is_removed_after_firing() {
        let mut list = vec![r(1, 100, Repeat::Once), r(2, 200, Repeat::Once)];
        let fired = take_due(&mut list, 150);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].id, 1);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, 2);
    }

    #[test]
    fn nothing_due_changes_nothing() {
        let mut list = vec![r(1, 100, Repeat::Daily)];
        assert!(take_due(&mut list, 99).is_empty());
        assert_eq!(list[0].at, 100);
    }

    #[test]
    fn daily_moves_to_next_day() {
        let mut list = vec![r(1, 1000, Repeat::Daily)];
        let fired = take_due(&mut list, 1000);
        assert_eq!(fired.len(), 1);
        assert_eq!(list[0].at, 1000 + DAY_MS);
    }

    #[test]
    fn missed_repeats_fire_once_and_skip_to_the_future() {
        // 3 日半ぶりに起動した
        let mut list = vec![r(1, 1000, Repeat::Daily)];
        let now = 1000 + 3 * DAY_MS + DAY_MS / 2;
        assert_eq!(take_due(&mut list, now).len(), 1);
        assert_eq!(list[0].at, 1000 + 4 * DAY_MS);
        assert!(list[0].at > now);
    }

    #[test]
    fn weekly_moves_by_a_week() {
        let mut list = vec![r(1, 0, Repeat::Weekly)];
        take_due(&mut list, 1);
        assert_eq!(list[0].at, 7 * DAY_MS);
    }

    #[test]
    fn fired_are_in_time_order() {
        let mut list = vec![r(1, 300, Repeat::Once), r(2, 100, Repeat::Once)];
        let ids: Vec<_> = take_due(&mut list, 500).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![2, 1]);
    }
}
