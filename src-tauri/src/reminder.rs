//! リマインダー：保存する内容、時刻になったかの判定、登録・一覧・削除の IPC、設定用ウィンドウ

use std::{
    sync::{atomic::Ordering, Arc},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_notification::NotificationExt;

use crate::{about, tray, Shared, MAIN_WINDOW};

pub const WINDOW: &str = "reminders";
/// 時刻になったかを調べる間隔
const CHECK_INTERVAL: Duration = Duration::from_secs(5);
/// 登録できる内容の長さ。ふきだしには収まらないぶんは省き、全文は通知センターに出す
pub const MAX_TEXT_CHARS: usize = 100;
/// ふきだしに出せる文字数（これを超えたら通知センターにも出す。src/speech.ts の BUBBLE_CHARS と同じ）
const BUBBLE_CHARS: usize = 30;
/// 「あとで知らせる」で、もう一度知らせるまでの時間
const SNOOZE_MS: i64 = 5 * 60 * 1000;
/// 鳴らす音（macOS 標準）
const SOUND_FILE: &str = "/System/Library/Sounds/Glass.aiff";
/// 登録できる件数
const MAX_REMINDERS: usize = 100;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    Once,
    Daily,
    Weekly,
    /// 2 週に 1 回
    Biweekly,
    /// 毎月、同じ日（Reminder::days に日付を入れる。31 日は短い月では月末）
    Monthly,
    /// 選んだ曜日だけ（Reminder::days のビット）
    Days,
}

impl Repeat {
    /// 一定の間隔でくり返すものの間隔。曜日指定は間隔が一定でないので None
    fn period_ms(self) -> Option<i64> {
        match self {
            Repeat::Once | Repeat::Days | Repeat::Monthly => None,
            Repeat::Daily => Some(DAY_MS),
            Repeat::Weekly => Some(7 * DAY_MS),
            Repeat::Biweekly => Some(14 * DAY_MS),
        }
    }
}

/// 曜日のビット（月曜 = bit 0 … 日曜 = bit 6）。曜日指定でこの 7 ビットのどれかを使う
pub const ALL_DAYS: u8 = 0b111_1111;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reminder {
    pub id: u64,
    pub text: String,
    /// 次に知らせる時刻（UNIX エポックからのミリ秒）
    pub at: i64,
    pub repeat: Repeat,
    /// repeat が Days のときの曜日のビット、Monthly のときの日付（古い設定ファイルにはない）
    #[serde(default)]
    pub days: u8,
}

/// `at` と同じ時刻（その地域の時計で）で、`mask` の曜日にあたる、`now` より後の最初の日時。
/// `from_next_day` なら `at` の翌日から探す
pub fn next_on_days<Tz: TimeZone>(
    tz: &Tz,
    at: i64,
    mask: u8,
    now: i64,
    from_next_day: bool,
) -> i64 {
    let Some(base) = tz.timestamp_millis_opt(at).single() else {
        return at;
    };
    let first = i64::from(from_next_day);
    for day in first..first + 15 {
        let naive = (base.date_naive() + ChronoDuration::days(day)).and_time(base.time());
        let Some(candidate) = tz.from_local_datetime(&naive).earliest() else {
            continue;
        };
        let ms = candidate.timestamp_millis();
        let bit = 1u8 << candidate.weekday().num_days_from_monday();
        if ms > now && mask & bit != 0 {
            return ms;
        }
    }
    at
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(y, m, 1)
        .and_then(|first| first.pred_opt())
        .map_or(28, |last| last.day())
}

/// `at` と同じ時刻で、毎月 `day` 日（その月にない日は月末）にあたる、`now` より後の最初の日時。
/// `from_next_month` なら `at` の翌月から探す
pub fn next_monthly<Tz: TimeZone>(
    tz: &Tz,
    at: i64,
    day: u32,
    now: i64,
    from_next_month: bool,
) -> i64 {
    let Some(base) = tz.timestamp_millis_opt(at).single() else {
        return at;
    };
    let first = u32::from(from_next_month);
    for k in first..first + 14 {
        let months = base.year() * 12 + base.month0() as i32 + k as i32;
        let (year, month) = (months.div_euclid(12), months.rem_euclid(12) as u32 + 1);
        let Some(date) = NaiveDate::from_ymd_opt(year, month, day.min(days_in_month(year, month)))
        else {
            continue;
        };
        let Some(candidate) = tz
            .from_local_datetime(&date.and_time(base.time()))
            .earliest()
        else {
            continue;
        };
        let ms = candidate.timestamp_millis();
        if ms > now {
            return ms;
        }
    }
    at
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
    take_due_in(&Local, list, now)
}

fn take_due_in<Tz: TimeZone>(tz: &Tz, list: &mut Vec<Reminder>, now: i64) -> Vec<Reminder> {
    let mut fired = Vec::new();
    list.retain_mut(|r| {
        if r.at > now {
            return true;
        }
        fired.push(r.clone());
        match (r.repeat, r.repeat.period_ms()) {
            (Repeat::Days, _) => {
                r.at = next_on_days(tz, r.at, r.days, now, true);
                true
            }
            (Repeat::Monthly, _) => {
                r.at = next_monthly(tz, r.at, u32::from(r.days), now, true);
                true
            }
            (_, Some(period)) => {
                r.at += ((now - r.at) / period + 1) * period;
                true
            }
            (_, None) => false,
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

/// 登録・編集の入力を確かめて、保存する値（内容、最初の日時、曜日）にする
fn validate(text: &str, at: i64, repeat: Repeat, days: u8) -> Result<(String, i64, u8), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("内容を入力してください".into());
    }
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(format!("内容は {MAX_TEXT_CHARS} 文字以内にしてください"));
    }
    let now = now_ms();
    match repeat {
        Repeat::Once if at <= now => Err("未来の日時を指定してください".into()),
        Repeat::Days => {
            let days = days & ALL_DAYS;
            if days == 0 {
                return Err("曜日を 1 つ以上選んでください".into());
            }
            // 選んだ日付が対象の曜日でなければ、次の対象の日にする
            Ok((text, next_on_days(&Local, at, days, now, false), days))
        }
        Repeat::Monthly => {
            // 最初に選んだ日付の「日」を、毎月の日として覚える
            let day = Local
                .timestamp_millis_opt(at)
                .single()
                .map_or(1, |dt| dt.day());
            Ok((text, at, day as u8))
        }
        _ => Ok((text, at, 0)),
    }
}

#[tauri::command]
pub fn add_reminder(
    app: AppHandle,
    shared: State<Arc<Shared>>,
    text: String,
    at: i64,
    repeat: Repeat,
    days: u8,
) -> Result<Vec<Reminder>, String> {
    let (text, at, days) = validate(&text, at, repeat, days)?;
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
            days,
        });
        result = Ok(sorted(s.reminders.clone()));
    });
    tray::refresh_reminders(&app);
    result
}

#[tauri::command]
pub fn update_reminder(
    app: AppHandle,
    shared: State<Arc<Shared>>,
    id: u64,
    text: String,
    at: i64,
    repeat: Repeat,
    days: u8,
) -> Result<Vec<Reminder>, String> {
    let (text, at, days) = validate(&text, at, repeat, days)?;
    let mut found = false;
    shared.store.update(|s| {
        if let Some(r) = s.reminders.iter_mut().find(|r| r.id == id) {
            *r = Reminder {
                id,
                text,
                at,
                repeat,
                days,
            };
            found = true;
        }
    });
    tray::refresh_reminders(&app);
    if found {
        Ok(sorted(shared.store.get().reminders))
    } else {
        Err("リマインダーが見つかりません（すでに削除されたか、通知済みです）".into())
    }
}

/// 「あとで知らせる」：同じ内容を、5 分後に 1 回だけ知らせる
#[tauri::command]
pub fn snooze_reminder(app: AppHandle, shared: State<Arc<Shared>>, text: String) {
    shared.store.update(|s| {
        let id = s.reminders.iter().map(|r| r.id).max().unwrap_or(0) + 1;
        s.reminders.push(Reminder {
            id,
            text,
            at: now_ms() + SNOOZE_MS,
            repeat: Repeat::Once,
            days: 0,
        });
    });
    tray::refresh_reminders(&app);
}

/// マスコットが見えない間（全画面アプリの上など）に、フロントエンドから呼ばれる
#[tauri::command]
pub fn notify_reminder(app: AppHandle, text: String) {
    notify(&app, &text);
}

#[tauri::command]
pub fn delete_reminder(app: AppHandle, shared: State<Arc<Shared>>, id: u64) -> Vec<Reminder> {
    shared.store.update(|s| s.reminders.retain(|r| r.id != id));
    tray::refresh_reminders(&app);
    sorted(shared.store.get().reminders)
}

/// macOS の通知センターに出す
fn notify(app: &AppHandle, text: &str) {
    let _ = app
        .notification()
        .builder()
        .title("リマインダー")
        .body(text)
        .show();
}

/// 音を鳴らす（待たずに戻る）
fn play_sound() {
    let _ = std::process::Command::new("afplay").arg(SOUND_FILE).spawn();
}

/// 時刻になったリマインダーをマスコットのふきだしへ送る。
/// マスコットを非表示にしている間と、ふきだしに収まらない長い内容は、通知センターにも出す
pub fn spawn_scheduler(app: AppHandle, shared: Arc<Shared>) {
    thread::spawn(move || loop {
        thread::sleep(CHECK_INTERVAL);
        let now = now_ms();
        if !shared.store.get().reminders.iter().any(|r| r.at <= now) {
            continue;
        }
        let mut fired = Vec::new();
        shared
            .store
            .update(|s| fired = take_due(&mut s.reminders, now));
        tray::refresh_reminders(&app);
        let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
            continue;
        };
        if shared.store.get().reminder_sound {
            play_sound();
        }
        let visible = shared.visible.load(Ordering::Relaxed);
        for r in fired {
            if !visible || r.text.chars().count() > BUBBLE_CHARS {
                notify(&app, &r.text);
            }
            if visible {
                let _ = window.emit("mascot-reminder", r.text);
            }
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
            days: 0,
        }
    }

    fn jst() -> chrono::FixedOffset {
        chrono::FixedOffset::east_opt(9 * 3600).unwrap()
    }

    /// JST の日時（ミリ秒）。2026-10-19 は月曜
    fn at_jst(day: u32, hour: u32, min: u32) -> i64 {
        jst()
            .with_ymd_and_hms(2026, 10, day, hour, min, 0)
            .unwrap()
            .timestamp_millis()
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

    #[test]
    fn days_skips_to_the_next_selected_weekday() {
        // 月曜 9:00 に鳴った、平日だけのリマインダー → 火曜 9:00
        let weekdays = 0b001_1111;
        let mon = at_jst(19, 9, 0);
        assert_eq!(
            next_on_days(&jst(), mon, weekdays, mon, true),
            at_jst(20, 9, 0)
        );
        // 金曜 9:00 に鳴ったら、土日を飛ばして月曜 9:00
        let fri = at_jst(23, 9, 0);
        assert_eq!(
            next_on_days(&jst(), fri, weekdays, fri, true),
            at_jst(26, 9, 0)
        );
    }

    #[test]
    fn days_first_date_moves_to_a_selected_weekday() {
        // 土曜を指定したが、対象は月・水 → 次の月曜
        let sat = at_jst(24, 8, 30);
        let mon_wed = 0b000_0101;
        assert_eq!(
            next_on_days(&jst(), sat, mon_wed, 0, false),
            at_jst(26, 8, 30)
        );
        // 対象の曜日ならそのまま
        let mon = at_jst(19, 8, 30);
        assert_eq!(next_on_days(&jst(), mon, mon_wed, 0, false), mon);
    }

    #[test]
    fn days_take_due_fires_once_and_moves_on() {
        let mut list = vec![Reminder {
            days: 0b001_1111,
            ..r(1, at_jst(23, 9, 0), Repeat::Days)
        }];
        // 金曜 9:00 に鳴り、次は月曜 9:00
        let fired = take_due_in(&jst(), &mut list, at_jst(23, 9, 1));
        assert_eq!(fired.len(), 1);
        assert_eq!(list[0].at, at_jst(26, 9, 0));
    }

    #[test]
    fn biweekly_moves_by_two_weeks() {
        let mut list = vec![r(1, 0, Repeat::Biweekly)];
        take_due(&mut list, 1);
        assert_eq!(list[0].at, 14 * DAY_MS);
    }

    #[test]
    fn monthly_keeps_the_day_and_clamps_to_month_end() {
        let jan31 = jst()
            .with_ymd_and_hms(2026, 1, 31, 9, 0, 0)
            .unwrap()
            .timestamp_millis();
        let feb28 = jst()
            .with_ymd_and_hms(2026, 2, 28, 9, 0, 0)
            .unwrap()
            .timestamp_millis();
        let mar31 = jst()
            .with_ymd_and_hms(2026, 3, 31, 9, 0, 0)
            .unwrap()
            .timestamp_millis();
        // 1/31 → 2/28（2 月は 28 日まで）→ 3/31（31 日に戻る）
        assert_eq!(next_monthly(&jst(), jan31, 31, jan31, true), feb28);
        assert_eq!(next_monthly(&jst(), feb28, 31, feb28, true), mar31);
    }

    #[test]
    fn monthly_crosses_the_year() {
        let dec15 = jst()
            .with_ymd_and_hms(2026, 12, 15, 8, 0, 0)
            .unwrap()
            .timestamp_millis();
        let jan15 = jst()
            .with_ymd_and_hms(2027, 1, 15, 8, 0, 0)
            .unwrap()
            .timestamp_millis();
        assert_eq!(next_monthly(&jst(), dec15, 15, dec15, true), jan15);
    }

    #[test]
    fn monthly_take_due_fires_once_after_a_long_absence() {
        let start = jst()
            .with_ymd_and_hms(2026, 1, 10, 9, 0, 0)
            .unwrap()
            .timestamp_millis();
        let now = jst()
            .with_ymd_and_hms(2026, 4, 20, 12, 0, 0)
            .unwrap()
            .timestamp_millis();
        let mut list = vec![Reminder {
            days: 10,
            ..r(1, start, Repeat::Monthly)
        }];
        assert_eq!(take_due_in(&jst(), &mut list, now).len(), 1);
        let next = jst()
            .with_ymd_and_hms(2026, 5, 10, 9, 0, 0)
            .unwrap()
            .timestamp_millis();
        assert_eq!(list[0].at, next);
    }
}
