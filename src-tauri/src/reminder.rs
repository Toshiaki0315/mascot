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

use crate::{about, pomodoro, settings::CountdownMode, tray, Shared, MAIN_WINDOW};

pub const WINDOW: &str = "reminders";
/// 画面がロックされている間などの、時刻になったかを調べる間隔
const PAUSED_CHECK_INTERVAL: Duration = Duration::from_secs(2);
/// 時刻になったかを調べる間隔（タイマーの最後の秒読みを、ほぼ 1 秒ごとに出せる細かさ）
const CHECK_INTERVAL: Duration = Duration::from_millis(500);
/// 登録できる内容の長さ。ふきだしには収まらないぶんは省き、全文は通知センターに出す
pub const MAX_TEXT_CHARS: usize = 100;
/// 「あとで知らせる」で、もう一度知らせるまでの時間
const SNOOZE_MS: i64 = 5 * 60 * 1000;
/// 鳴らす音（macOS 標準）
const SOUND_FILE: &str = "/System/Library/Sounds/Glass.aiff";
/// 登録できる件数
const MAX_REMINDERS: usize = 100;
/// タイマーの最長（分）
const MAX_TIMER_MIN: u32 = 24 * 60;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// `advance_days` で進める回数の上限（毎日のリマインダーなら、約 55 年分）
const MAX_ADVANCE_STEPS: i64 = 20_000;

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
    /// 何日おきにくり返すか。曜日指定と毎月は、日数が一定でないので None
    fn step_days(self) -> Option<i64> {
        match self {
            Repeat::Once | Repeat::Days | Repeat::Monthly => None,
            Repeat::Daily => Some(1),
            Repeat::Weekly => Some(7),
            Repeat::Biweekly => Some(14),
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
    /// 何分前に先に知らせるか（0 なら知らせない）
    #[serde(default)]
    pub lead_min: u32,
    /// 事前に知らせた時刻（`at` の値）。同じ予定で 2 度知らせないための記録
    #[serde(default)]
    pub lead_sent: i64,
    /// カウントダウンのタイマー（「タイマー」の一覧に出し、リマインダーの一覧には出さない）
    #[serde(default)]
    pub timer: bool,
}

impl Reminder {
    pub fn new(id: u64, text: String, at: i64, repeat: Repeat, days: u8) -> Self {
        Self {
            id,
            text,
            at,
            repeat,
            days,
            lead_min: 0,
            lead_sent: 0,
            timer: false,
        }
    }
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

/// 現地の時計で、`at` と同じ時刻の、`step_days` 日おきの日のうち、`now` より後の最初の日時。
/// 24 時間ずつ足すと、夏時間の切り替えをまたいだときに、現地の時刻が 1 時間ずれるので、
/// 現地のカレンダーの日付を進めて、時刻を合わせ直す
pub fn advance_days<Tz: TimeZone>(tz: &Tz, at: i64, step_days: i64, now: i64) -> i64 {
    let Some(base) = tz.timestamp_millis_opt(at).single() else {
        return at + step_days * DAY_MS;
    };
    // 何年も止めていても、現実的な回数で終わるよう、上限を置く
    for k in 1..=MAX_ADVANCE_STEPS {
        let naive = (base.date_naive() + ChronoDuration::days(step_days * k)).and_time(base.time());
        // 夏時間の切り替えで、その時刻が存在しない日（時計が飛ぶ）は、1 時間後にずらす
        let local = tz.from_local_datetime(&naive).earliest().or_else(|| {
            tz.from_local_datetime(&(naive + ChronoDuration::hours(1)))
                .earliest()
        });
        if let Some(local) = local {
            let ms = local.timestamp_millis();
            if ms > now {
                return ms;
            }
        }
    }
    at + step_days * DAY_MS * MAX_ADVANCE_STEPS
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
        match (r.repeat, r.repeat.step_days()) {
            (Repeat::Days, _) => {
                r.at = next_on_days(tz, r.at, r.days, now, true);
                true
            }
            (Repeat::Monthly, _) => {
                r.at = next_monthly(tz, r.at, u32::from(r.days), now, true);
                true
            }
            (_, Some(step)) => {
                r.at = advance_days(tz, r.at, step, now);
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

/// リマインダー（タイマーとスヌーズも含む）を、もう 1 つ足せるか
fn has_room(count: usize) -> bool {
    count < MAX_REMINDERS
}

fn too_many() -> String {
    format!("リマインダーとタイマーは、合わせて {MAX_REMINDERS} 件までです。いらないものを消してください")
}

/// 事前に知らせる時間は、最長 1 日前まで
fn check_lead(lead_min: u32) -> Result<(), String> {
    if lead_min > 24 * 60 {
        return Err("事前に知らせる時間は 1 日前までにしてください".into());
    }
    Ok(())
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
    lead_min: u32,
) -> Result<Vec<Reminder>, String> {
    let (text, at, days) = validate(&text, at, repeat, days)?;
    check_lead(lead_min)?;
    let mut result = Err(too_many());
    shared.store.update(|s| {
        if !has_room(s.reminders.len()) {
            return;
        }
        let id = s.reminders.iter().map(|r| r.id).max().unwrap_or(0) + 1;
        s.reminders.push(Reminder {
            lead_min,
            ..Reminder::new(id, text, at, repeat, days)
        });
        result = Ok(sorted(s.reminders.clone()));
    });
    tray::refresh_reminders(&app);
    result
}

// Tauri のコマンドは、引数の名前でフロントエンドから値を受け取るので、数が多くなる
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn update_reminder(
    app: AppHandle,
    shared: State<Arc<Shared>>,
    id: u64,
    text: String,
    at: i64,
    repeat: Repeat,
    days: u8,
    lead_min: u32,
) -> Result<Vec<Reminder>, String> {
    let (text, at, days) = validate(&text, at, repeat, days)?;
    check_lead(lead_min)?;
    let mut found = false;
    shared.store.update(|s| {
        if let Some(r) = s.reminders.iter_mut().find(|r| r.id == id) {
            *r = Reminder {
                lead_min,
                ..Reminder::new(id, text, at, repeat, days)
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
pub fn snooze_reminder(
    app: AppHandle,
    shared: State<Arc<Shared>>,
    text: String,
) -> Result<(), String> {
    let mut added = false;
    shared.store.update(|s| {
        if !has_room(s.reminders.len()) {
            return;
        }
        let id = s.reminders.iter().map(|r| r.id).max().unwrap_or(0) + 1;
        s.reminders.push(Reminder::new(
            id,
            text,
            now_ms() + SNOOZE_MS,
            Repeat::Once,
            0,
        ));
        added = true;
    });
    tray::refresh_reminders(&app);
    if added {
        Ok(())
    } else {
        Err(too_many())
    }
}

/// カウントダウンのタイマーを始める（時間が来たら、ふきだしと通知で知らせる）
pub fn add_timer(
    app: &AppHandle,
    shared: &Shared,
    minutes: u32,
    label: &str,
) -> Result<Vec<Reminder>, String> {
    if !(1..=MAX_TIMER_MIN).contains(&minutes) {
        return Err(format!("タイマーは 1〜{MAX_TIMER_MIN} 分にしてください"));
    }
    let label = label.trim();
    if label.chars().count() > MAX_TEXT_CHARS {
        return Err(format!("名前は {MAX_TEXT_CHARS} 文字以内にしてください"));
    }
    let text = if label.is_empty() {
        format!("{minutes} 分のタイマーが終わりました")
    } else {
        format!("{label}：時間です")
    };
    let mut added = false;
    shared.store.update(|s| {
        if !has_room(s.reminders.len()) {
            return;
        }
        let id = s.reminders.iter().map(|r| r.id).max().unwrap_or(0) + 1;
        s.reminders.push(Reminder {
            timer: true,
            ..Reminder::new(
                id,
                text,
                now_ms() + i64::from(minutes) * 60_000,
                Repeat::Once,
                0,
            )
        });
        added = true;
    });
    if !added {
        return Err(too_many());
    }
    tray::refresh_timers(app);
    Ok(sorted(shared.store.get().reminders))
}

#[tauri::command]
pub fn start_timer(
    app: AppHandle,
    shared: State<Arc<Shared>>,
    minutes: u32,
    label: String,
) -> Result<Vec<Reminder>, String> {
    add_timer(&app, &shared, minutes, &label)
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
    // 音の出力先がないときなどに afplay が出すエラーで、ターミナルを汚さない。
    // 終わりを待つ係のスレッドを立てて、終わったプロセスが残らないようにする
    let child = std::process::Command::new("afplay")
        .arg(SOUND_FILE)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    if let Ok(mut child) = child {
        thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

/// ふきだしへ送る知らせ
#[derive(Clone, Serialize)]
struct Announce {
    text: String,
    /// 「5 分後」のボタンを付けるか
    snooze: bool,
}

/// ふきだしで知らせられる状態か。マスコットを隠している間と、画面がロックされている・
/// ディスプレイが消えている間は、誰も見ていない。ふきだしに送ると、自動で閉じる 5 分のうちに
/// 気づけないので、通知センターに出す（通知は残る）
fn bubble_visible(mascot_visible: bool, screen_paused: bool) -> bool {
    mascot_visible && !screen_paused
}

/// マスコットのふきだしで知らせる。マスコットを非表示にしている間は、通知センターに出す
pub fn announce(app: &AppHandle, shared: &Shared, text: &str, snooze: bool) {
    let visible = bubble_visible(shared.visible.load(Ordering::Relaxed), shared.paused());
    if !visible {
        notify(app, text);
    }
    if visible {
        if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
            let _ = window.emit(
                "mascot-reminder",
                Announce {
                    text: text.to_string(),
                    snooze,
                },
            );
        }
    }
}

/// タイマーの最後の秒読みを始める残り時間（ミリ秒）
const COUNTDOWN_FROM_MS: i64 = 10_000;
/// タイマーごとに、秒読みをふきだしに出すかを決める確率（%）
const COUNTDOWN_CHANCE_PERCENT: u64 = 50;

/// 残り時間から、いま出す秒読みの数字（10 → 1）。残りがなければ（時間が来たあと）None
pub fn countdown_number(remaining_ms: i64) -> Option<u32> {
    if remaining_ms <= 0 || remaining_ms > COUNTDOWN_FROM_MS {
        return None;
    }
    Some(((remaining_ms + 999) / 1000) as u32)
}

/// 設定と、0〜99 の乱数から、秒読みを出すかを決める
pub fn show_countdown(mode: CountdownMode, roll: u64) -> bool {
    match mode {
        CountdownMode::Always => true,
        CountdownMode::Never => false,
        CountdownMode::Random => roll % 100 < COUNTDOWN_CHANCE_PERCENT,
    }
}

/// 標準ライブラリだけで作る乱数（毎回違う値になる）
fn random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
}

/// 秒読みの数字を、ふつうのおしゃべりのふきだしで出す（マスコットが眠っていても出す）。
/// マスコットを隠している間は出さない
fn say_countdown(app: &AppHandle, shared: &Shared, number: u32) {
    if !shared.visible.load(Ordering::Relaxed) {
        return;
    }
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.emit("mascot-countdown", number.to_string());
    }
}

/// 短いひとことを、ふつうのふきだしで出す（コピーしたよ、など）。見えない間は何もしない
pub fn toast(app: &AppHandle, text: &str) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.emit("mascot-toast", text);
    }
}

/// 事前に知らせる時刻になったリマインダーを探して、知らせた印を付ける
fn take_leads(reminders: &mut [Reminder], now: i64) -> Vec<String> {
    let mut texts = Vec::new();
    for r in reminders.iter_mut() {
        let lead_at = r.at - i64::from(r.lead_min) * 60_000;
        if r.lead_min > 0 && !r.timer && r.lead_sent != r.at && now >= lead_at && now < r.at {
            r.lead_sent = r.at;
            texts.push(format!("あと {} 分：{}", r.lead_min, r.text));
        }
    }
    texts
}

/// 0.5 秒ごとに、時刻になったリマインダーとタイマー、事前に知らせるもの、ポモドーロの区間の終わり、
/// タイマーの最後の秒読み（10 → 1）を調べる。秒読みを出すかは、設定（ランダム／いつも出す／出さない）で決める。ランダムのときは、タイマーごとに半々で決める
pub fn spawn_scheduler(app: AppHandle, shared: Arc<Shared>) {
    thread::spawn(move || {
        // タイマーの ID → （秒読みを出すか, 最後に出した数字）
        let mut countdowns: std::collections::HashMap<u64, (bool, u32)> =
            std::collections::HashMap::new();
        loop {
            // 画面がロックされている・ディスプレイが消えている間は、確認を粗くする
            // （秒読みは見る人がいないので不要。リマインダーとタイマーは、2 秒以内に知らせる）
            thread::sleep(if shared.paused() {
                PAUSED_CHECK_INTERVAL
            } else {
                CHECK_INTERVAL
            });
            let now = now_ms();

            let settings = shared.store.get();
            let mode = settings.countdown_mode;
            let timers: Vec<(u64, i64)> = settings
                .reminders
                .iter()
                .filter(|r| r.timer)
                .map(|r| (r.id, r.at))
                .collect();
            countdowns.retain(|id, _| timers.iter().any(|(t, _)| t == id));
            for (id, at) in timers {
                let Some(number) = countdown_number(at - now) else {
                    continue;
                };
                let entry = countdowns
                    .entry(id)
                    .or_insert_with(|| (show_countdown(mode, random_u64()), 0));
                if entry.0 && entry.1 != number {
                    entry.1 = number;
                    say_countdown(&app, &shared, number);
                }
            }
            let mut messages: Vec<(String, bool)> = Vec::new();
            // 離席で止めたことを知らせるだけのときは、音を鳴らさない
            let mut sound = false;

            let any_lead = shared
                .store
                .get()
                .reminders
                .iter()
                .any(|r| r.lead_min > 0 && !r.timer && r.lead_sent != r.at && now < r.at);
            if any_lead {
                let mut leads = Vec::new();
                shared
                    .store
                    .update(|s| leads = take_leads(&mut s.reminders, now));
                sound |= !leads.is_empty() && settings.reminder_sound;
                messages.extend(leads.into_iter().map(|t| (t, false)));
            }

            if shared.store.get().reminders.iter().any(|r| r.at <= now) {
                let mut fired = Vec::new();
                shared
                    .store
                    .update(|s| fired = take_due(&mut s.reminders, now));
                tray::refresh_reminders(&app);
                tray::refresh_timers(&app);
                sound |= fired.iter().any(|r| {
                    if r.timer {
                        settings.timer_sound
                    } else {
                        settings.reminder_sound
                    }
                });
                messages.extend(fired.into_iter().map(|r| (r.text, !r.timer)));
            }

            if let Some((text, with_sound)) = pomodoro::tick(&app, &shared, now) {
                sound |= with_sound && settings.pomodoro_sound;
                messages.push((text, false));
            }

            if messages.is_empty() {
                continue;
            }
            if sound {
                play_sound();
            }
            for (text, snooze) in messages {
                announce(&app, &shared, &text, snooze);
            }
        }
    });
}

/// 設定ウィンドウを開く（開いていれば前面に出す）。`tab` は最初に出す見出し。メインスレッドから呼ぶこと
pub fn open_window(app: &AppHandle, tab: &str) {
    about::activate_app();
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("tools-tab", tab);
        return;
    }
    let url = format!("reminders.html?tab={tab}");
    let window = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::App(url.into()))
        .title("Desktop Mascot")
        .inner_size(460.0, 620.0)
        .min_inner_size(360.0, 420.0)
        // 画像のファイルのドラッグ＆ドロップ（じぶんの絵）を、ページが直接受け取れるようにする
        .disable_drag_drop_handler()
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
        Reminder::new(id, format!("r{id}"), at, repeat, 0)
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
        let fired = take_due_in(&jst(), &mut list, 150);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].id, 1);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, 2);
    }

    #[test]
    fn nothing_due_changes_nothing() {
        let mut list = vec![r(1, 100, Repeat::Daily)];
        assert!(take_due_in(&jst(), &mut list, 99).is_empty());
        assert_eq!(list[0].at, 100);
    }

    #[test]
    fn daily_moves_to_next_day() {
        let mut list = vec![r(1, 1000, Repeat::Daily)];
        let fired = take_due_in(&jst(), &mut list, 1000);
        assert_eq!(fired.len(), 1);
        assert_eq!(list[0].at, 1000 + DAY_MS);
    }

    #[test]
    fn missed_repeats_fire_once_and_skip_to_the_future() {
        // 3 日半ぶりに起動した
        let mut list = vec![r(1, 1000, Repeat::Daily)];
        let now = 1000 + 3 * DAY_MS + DAY_MS / 2;
        assert_eq!(take_due_in(&jst(), &mut list, now).len(), 1);
        assert_eq!(list[0].at, 1000 + 4 * DAY_MS);
        assert!(list[0].at > now);
    }

    #[test]
    fn weekly_moves_by_a_week() {
        let mut list = vec![r(1, 0, Repeat::Weekly)];
        take_due_in(&jst(), &mut list, 1);
        assert_eq!(list[0].at, 7 * DAY_MS);
    }

    #[test]
    fn fired_are_in_time_order() {
        let mut list = vec![r(1, 300, Repeat::Once), r(2, 100, Repeat::Once)];
        let ids: Vec<_> = take_due_in(&jst(), &mut list, 500)
            .iter()
            .map(|r| r.id)
            .collect();
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
        take_due_in(&jst(), &mut list, 1);
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

    #[test]
    fn lead_notice_fires_once_inside_the_lead_window() {
        let mut list = vec![Reminder {
            lead_min: 10,
            ..r(1, 100 * 60_000, Repeat::Once)
        }];
        // 15 分前はまだ
        assert!(take_leads(&mut list, 85 * 60_000).is_empty());
        // 10 分前になったら 1 回だけ
        let texts = take_leads(&mut list, 90 * 60_000);
        assert_eq!(texts, vec!["あと 10 分：r1".to_string()]);
        assert!(take_leads(&mut list, 95 * 60_000).is_empty());
        // 時刻を過ぎたら出さない
        let mut late = vec![Reminder {
            lead_min: 10,
            ..r(2, 100 * 60_000, Repeat::Once)
        }];
        assert!(take_leads(&mut late, 100 * 60_000).is_empty());
    }

    #[test]
    fn lead_notice_fires_again_for_the_next_repeat() {
        let day = DAY_MS;
        let mut list = vec![Reminder {
            lead_min: 5,
            ..r(1, 10 * 60_000, Repeat::Daily)
        }];
        assert_eq!(take_leads(&mut list, 5 * 60_000).len(), 1);
        // 翌日の分へ進んだあと（take_due と同じ動き）
        list[0].at += day;
        assert!(take_leads(&mut list, day).is_empty());
        assert_eq!(take_leads(&mut list, day + 5 * 60_000).len(), 1);
    }

    #[test]
    fn timers_never_get_lead_notices() {
        let mut list = vec![Reminder {
            lead_min: 5,
            timer: true,
            ..r(1, 10 * 60_000, Repeat::Once)
        }];
        assert!(take_leads(&mut list, 6 * 60_000).is_empty());
    }

    #[test]
    fn countdown_numbers_run_from_ten_to_one() {
        assert_eq!(countdown_number(10_001), None);
        assert_eq!(countdown_number(10_000), Some(10));
        assert_eq!(countdown_number(9_001), Some(10));
        assert_eq!(countdown_number(9_000), Some(9));
        assert_eq!(countdown_number(1_001), Some(2));
        assert_eq!(countdown_number(1_000), Some(1));
        assert_eq!(countdown_number(1), Some(1));
        assert_eq!(countdown_number(0), None);
        assert_eq!(countdown_number(-500), None);
    }

    #[test]
    fn countdown_random_shows_for_about_half_of_the_rolls() {
        let shown = (0..100)
            .filter(|&n| show_countdown(CountdownMode::Random, n))
            .count();
        assert_eq!(shown, 50);
        assert!(show_countdown(CountdownMode::Random, 0));
        assert!(!show_countdown(CountdownMode::Random, 99));
    }

    #[test]
    fn countdown_always_and_never_ignore_the_roll() {
        for roll in [0, 49, 50, 99, 12345] {
            assert!(show_countdown(CountdownMode::Always, roll));
            assert!(!show_countdown(CountdownMode::Never, roll));
        }
    }

    // 夏時間のある地域（ニューヨーク）で、現地の時刻が 1 時間ずれないことを確かめる
    fn ny(y: i32, m: u32, d: u32, h: u32) -> i64 {
        chrono_tz::America::New_York
            .with_ymd_and_hms(y, m, d, h, 0, 0)
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn daily_keeps_the_local_time_across_the_spring_dst_change() {
        // 2026-03-08 に夏時間が始まる（その日は 23 時間しかない）
        let mut list = vec![r(1, ny(2026, 3, 7, 9), Repeat::Daily)];
        let fired = take_due_in(&chrono_tz::America::New_York, &mut list, ny(2026, 3, 7, 9));
        assert_eq!(fired.len(), 1);
        assert_eq!(list[0].at, ny(2026, 3, 8, 9));
        assert_eq!(ny(2026, 3, 8, 9) - ny(2026, 3, 7, 9), 23 * 60 * 60 * 1000);
    }

    #[test]
    fn weekly_keeps_the_local_time_across_the_fall_dst_change() {
        // 2026-11-01 に夏時間が終わる（その日は 25 時間ある）
        let mut list = vec![r(1, ny(2026, 10, 28, 9), Repeat::Weekly)];
        take_due_in(
            &chrono_tz::America::New_York,
            &mut list,
            ny(2026, 10, 28, 9),
        );
        assert_eq!(list[0].at, ny(2026, 11, 4, 9));
    }

    #[test]
    fn advance_days_skips_a_local_time_that_does_not_exist() {
        // 夏時間が始まる日の 2:30 は存在しない。1 時間ずらして 3:30 にする
        let at = chrono_tz::America::New_York
            .with_ymd_and_hms(2026, 3, 7, 2, 30, 0)
            .unwrap()
            .timestamp_millis();
        let next = advance_days(&chrono_tz::America::New_York, at, 1, at);
        let want = chrono_tz::America::New_York
            .with_ymd_and_hms(2026, 3, 8, 3, 30, 0)
            .unwrap()
            .timestamp_millis();
        assert_eq!(next, want);
    }

    #[test]
    fn a_long_absence_still_finds_the_next_day_quickly() {
        // 3 年ぶりに起動しても、毎日のリマインダーは、次の日時に進む
        let at = ny(2023, 1, 10, 9);
        let now = ny(2026, 1, 10, 12);
        let next = advance_days(&chrono_tz::America::New_York, at, 1, now);
        assert_eq!(next, ny(2026, 1, 11, 9));
    }

    #[test]
    fn the_reminder_cap_counts_everything() {
        assert!(has_room(0));
        assert!(has_room(MAX_REMINDERS - 1));
        assert!(!has_room(MAX_REMINDERS));
        assert!(!has_room(MAX_REMINDERS + 5));
        assert!(too_many().contains(&MAX_REMINDERS.to_string()));
    }

    #[test]
    fn bubbles_are_only_for_a_visible_mascot_and_an_active_screen() {
        assert!(bubble_visible(true, false));
        assert!(!bubble_visible(false, false));
        assert!(!bubble_visible(true, true));
        assert!(!bubble_visible(false, true));
    }
}
