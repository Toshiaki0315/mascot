//! メニューバーのトレイアイコンと設定メニュー

use std::sync::{atomic::Ordering, Arc, Mutex};

use chrono::{Datelike, Local, TimeZone, Timelike};

use tauri::{
    image::Image,
    menu::{CheckMenuItem, ContextMenu, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewWindow, Wry,
};

use crate::{
    about, bubble, emit_settings, notes, pomodoro,
    reminder::{self, Reminder, Repeat},
    save_current_position,
    settings::Character,
    Shared, MAIN_WINDOW,
};

/// 中身（直近の予定やメモの行）を後から作り直すサブメニュー
struct DynamicMenu {
    menu: Submenu<Wry>,
    rows: Mutex<Vec<MenuItem<Wry>>>,
}

impl DynamicMenu {
    fn new(menu: Submenu<Wry>) -> Self {
        Self {
            menu,
            rows: Mutex::new(Vec::new()),
        }
    }

    /// 前に作った行を消して、新しい行を並べる
    fn replace(&self, new_rows: Vec<MenuItem<Wry>>) {
        let mut rows = self.rows.lock().unwrap();
        for row in rows.drain(..) {
            let _ = self.menu.remove(&row);
        }
        for row in new_rows {
            if self.menu.append(&row).is_ok() {
                rows.push(row);
            }
        }
    }
}

/// メニュー項目のチェック状態を後から書き換えるために持っておく
struct TrayItems {
    characters: Vec<(Character, CheckMenuItem<Wry>)>,
    /// 「リマインダー」のサブメニューと、その中の直近の予定の行（予定が変わるたびに作り直す）
    reminder_menu: Submenu<Wry>,
    reminder_rows: Mutex<Vec<MenuItem<Wry>>>,
    timers: DynamicMenu,
    pomodoro: DynamicMenu,
    notes: DynamicMenu,
    /// トレイと、マスコットの右クリックで共有する
    menu: Menu<Wry>,
}

pub fn setup(app: &tauri::App, shared: &Arc<Shared>) -> tauri::Result<()> {
    let settings = shared.store.get();

    let about = MenuItem::with_id(
        app,
        "about",
        format!("{} について", app.package_info().name),
        true,
        None::<&str>,
    )?;
    let toggle = MenuItem::with_id(app, "toggle", "表示 / 非表示", true, None::<&str>)?;
    // 機能ごとのサブメニューは、どれも同じ並びにそろえる：
    //   ① 〇〇の画面を開く…  ──  ② すぐ使う操作  ──  ③ 直近の一覧（予定が変わるたびに作り直す）
    let open_item = |id: &str, label: &str| MenuItem::with_id(app, id, label, true, None::<&str>);

    let reminder_menu = Submenu::with_items(
        app,
        "リマインダー",
        true,
        &[
            &open_item("reminders", "リマインダーの画面を開く…")?,
            &PredefinedMenuItem::separator(app)?,
        ],
    )?;
    let timer_menu = Submenu::with_items(
        app,
        "タイマー",
        true,
        &[
            &open_item("tools-timers", "タイマーの画面を開く…")?,
            &PredefinedMenuItem::separator(app)?,
            &open_item("timer-3", "3 分タイマー")?,
            &open_item("timer-5", "5 分タイマー")?,
            &open_item("timer-10", "10 分タイマー")?,
            &PredefinedMenuItem::separator(app)?,
        ],
    )?;
    let pomodoro_menu = Submenu::with_items(
        app,
        "ポモドーロ",
        true,
        &[
            &open_item("tools-pomodoro", "ポモドーロの画面を開く…")?,
            &PredefinedMenuItem::separator(app)?,
        ],
    )?;
    let notes_menu = Submenu::with_items(
        app,
        "メモ",
        true,
        &[
            &open_item("tools-notes", "メモの画面を開く…")?,
            &PredefinedMenuItem::separator(app)?,
            &open_item("notes-save-clip", "クリップボードをメモに保存")?,
            &PredefinedMenuItem::separator(app)?,
        ],
    )?;
    let weather_menu = Submenu::with_items(
        app,
        "今日の天気",
        true,
        &[
            &open_item("tools-weather", "天気の画面を開く…")?,
            &PredefinedMenuItem::separator(app)?,
            &open_item("weather-now", "今日の天気を見る")?,
        ],
    )?;
    let characters = Character::ALL
        .iter()
        .map(|&character| {
            CheckMenuItem::with_id(
                app,
                character.id(),
                character.label(),
                true,
                settings.character == character,
                None::<&str>,
            )
            .map(|item| (character, item))
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let character_menu = Submenu::with_items(
        app,
        "キャラクター",
        true,
        &characters
            .iter()
            .map(|(_, item)| item as &dyn tauri::menu::IsMenuItem<Wry>)
            .collect::<Vec<_>>(),
    )?;

    let quit = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &about,
            &PredefinedMenuItem::separator(app)?,
            &toggle,
            &PredefinedMenuItem::separator(app)?,
            &reminder_menu,
            &timer_menu,
            &pomodoro_menu,
            &notes_menu,
            &weather_menu,
            &PredefinedMenuItem::separator(app)?,
            &character_menu,
            &PredefinedMenuItem::separator(app)?,
            &open_item("tools-general", "設定…")?,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    app.manage(TrayItems {
        characters,
        reminder_menu,
        reminder_rows: Mutex::new(Vec::new()),
        timers: DynamicMenu::new(timer_menu),
        pomodoro: DynamicMenu::new(pomodoro_menu),
        notes: DynamicMenu::new(notes_menu),
        menu: menu.clone(),
    });

    // 白黒のテンプレート画像にすると、ライト/ダークどちらのメニューバーにもなじむ
    let icon = Image::from_bytes(include_bytes!("../icons/tray-icon.png"))?;
    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Desktop Mascot")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| on_menu(app, event.id.as_ref()))
        .build(app)?;
    refresh_reminders(app.handle());
    refresh_timers(app.handle());
    refresh_pomodoro(app.handle());
    refresh_notes(app.handle());
    Ok(())
}

/// メニューに出す直近のリマインダーの件数
const UPCOMING: usize = 5;
/// メニューの 1 行に出す内容の文字数
const LABEL_CHARS: usize = 14;

fn reminder_label(r: &Reminder) -> String {
    let when = match Local.timestamp_millis_opt(r.at).single() {
        Some(dt) => format!(
            "{}/{} {:02}:{:02}",
            dt.month(),
            dt.day(),
            dt.hour(),
            dt.minute()
        ),
        None => String::new(),
    };
    let mark = if r.repeat == Repeat::Once { "" } else { " ↻" };
    let chars: Vec<char> = r.text.chars().collect();
    let text = if chars.len() > LABEL_CHARS {
        format!("{}…", chars[..LABEL_CHARS - 1].iter().collect::<String>())
    } else {
        r.text.clone()
    };
    format!("{when}{mark}  {text}")
}

/// 「リマインダー」サブメニューの、直近の予定の行を作り直す。
/// 予定を足した・直した・消した・知らせたときに呼ぶ
pub fn refresh_reminders(app: &AppHandle) {
    let (Some(items), Some(shared)) =
        (app.try_state::<TrayItems>(), app.try_state::<Arc<Shared>>())
    else {
        return;
    };
    let mut list = shared.store.get().reminders;
    list.retain(|r| !r.timer);
    list.sort_by_key(|r| (r.at, r.id));

    let mut rows = items.reminder_rows.lock().unwrap();
    for row in rows.drain(..) {
        let _ = items.reminder_menu.remove(&row);
    }
    let mut new_rows = Vec::new();
    if list.is_empty() {
        new_rows.extend(MenuItem::with_id(
            app,
            "reminders-none",
            "予定はありません",
            false,
            None::<&str>,
        ));
    }
    for r in list.iter().take(UPCOMING) {
        // 押すと、追加・編集のウィンドウが開く
        new_rows.extend(MenuItem::with_id(
            app,
            format!("reminders-item-{}", r.id),
            reminder_label(r),
            true,
            None::<&str>,
        ));
    }
    if list.len() > UPCOMING {
        new_rows.extend(MenuItem::with_id(
            app,
            "reminders-more",
            format!("ほか {} 件", list.len() - UPCOMING),
            false,
            None::<&str>,
        ));
    }
    for row in new_rows {
        if items.reminder_menu.append(&row).is_ok() {
            rows.push(row);
        }
    }
}

fn clock(at: i64) -> String {
    match Local.timestamp_millis_opt(at).single() {
        Some(dt) => format!("{:02}:{:02}", dt.hour(), dt.minute()),
        None => String::new(),
    }
}

fn short(text: &str, max: usize) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let chars: Vec<char> = flat.chars().collect();
    if chars.len() > max {
        format!("{}…", chars[..max - 1].iter().collect::<String>())
    } else {
        flat
    }
}

/// 「タイマー」サブメニューの、動いているタイマーの行を作り直す
pub fn refresh_timers(app: &AppHandle) {
    let (Some(items), Some(shared)) =
        (app.try_state::<TrayItems>(), app.try_state::<Arc<Shared>>())
    else {
        return;
    };
    let settings = shared.store.get();
    let mut rows = Vec::new();
    let mut timers: Vec<&Reminder> = settings.reminders.iter().filter(|r| r.timer).collect();
    timers.sort_by_key(|r| (r.at, r.id));
    if timers.is_empty() {
        rows.extend(MenuItem::with_id(
            app,
            "timer-none",
            "動いているタイマーはありません",
            false,
            None::<&str>,
        ));
    }
    for r in timers {
        // 押すと、タイマーの画面が開く
        rows.extend(MenuItem::with_id(
            app,
            format!("tools-timers-item-{}", r.id),
            format!("⏱ {} まで  {}", clock(r.at), short(&r.text, 14)),
            true,
            None::<&str>,
        ));
    }
    items.timers.replace(rows);
}

/// 「ポモドーロ」サブメニューの、いまの状態と、始める・止める行を作り直す
pub fn refresh_pomodoro(app: &AppHandle) {
    let (Some(items), Some(shared)) =
        (app.try_state::<TrayItems>(), app.try_state::<Arc<Shared>>())
    else {
        return;
    };
    let settings = shared.store.get();
    let mut rows = Vec::new();
    match settings.pomodoro {
        Some(p) => {
            rows.extend(MenuItem::with_id(
                app,
                "pomo-status",
                format!(
                    "🍅 {} {} まで（{} 回）",
                    p.phase.label(),
                    clock(p.ends_at),
                    p.cycle
                ),
                false,
                None::<&str>,
            ));
            rows.extend(MenuItem::with_id(
                app,
                "pomo-stop",
                "ポモドーロを止める",
                true,
                None::<&str>,
            ));
        }
        None => {
            let cfg = settings.pomodoro_cfg;
            rows.extend(MenuItem::with_id(
                app,
                "pomo-start",
                format!(
                    "ポモドーロを始める（{} 分 / {} 分）",
                    cfg.work_min, cfg.break_min
                ),
                true,
                None::<&str>,
            ));
        }
    }
    items.pomodoro.replace(rows);
}

/// 「メモ」サブメニューの、直近のメモの行を作り直す
pub fn refresh_notes(app: &AppHandle) {
    let (Some(items), Some(shared)) =
        (app.try_state::<TrayItems>(), app.try_state::<Arc<Shared>>())
    else {
        return;
    };
    let mut list = shared.store.get().notes;
    list.sort_by_key(|n| std::cmp::Reverse((n.at, n.id)));
    let mut rows = Vec::new();
    if list.is_empty() {
        rows.extend(MenuItem::with_id(
            app,
            "note-none",
            "メモはありません",
            false,
            None::<&str>,
        ));
    }
    for n in list.iter().take(UPCOMING) {
        // 押すと、クリップボードにコピーする
        rows.extend(MenuItem::with_id(
            app,
            format!("note-item-{}", n.id),
            short(&n.text, 20),
            true,
            None::<&str>,
        ));
    }
    if list.len() > UPCOMING {
        rows.extend(MenuItem::with_id(
            app,
            "note-more",
            format!("ほか {} 件", list.len() - UPCOMING),
            false,
            None::<&str>,
        ));
    }
    items.notes.replace(rows);
}

/// マスコットの上に、トレイと同じメニューを出す
pub fn popup(window: &WebviewWindow) {
    let items = window.app_handle().state::<TrayItems>();
    let _ = items.menu.popup(window.as_ref().window());
}

fn on_menu(app: &AppHandle, id: &str) {
    let shared = app.state::<Arc<Shared>>();
    let items = app.state::<TrayItems>();
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };

    match id {
        "about" => {
            let info = app.package_info();
            about::show(&info.name, &info.version.to_string());
        }
        id if id.starts_with("reminders") => reminder::open_window(app, "reminders"),
        id if id.starts_with("tools-timers") => reminder::open_window(app, "timers"),
        "tools-general" => reminder::open_window(app, "general"),
        "tools-pomodoro" => reminder::open_window(app, "pomodoro"),
        "tools-weather" => reminder::open_window(app, "weather"),
        "tools-notes" => reminder::open_window(app, "notes"),
        "timer-3" | "timer-5" | "timer-10" => {
            let minutes: u32 = id["timer-".len()..].parse().unwrap_or(3);
            let _ = reminder::add_timer(app, &shared, minutes, "");
            reminder::toast(app, &format!("{minutes} 分のタイマーを始めたよ"));
        }
        "pomo-start" => {
            pomodoro::start_now(app, &shared);
            reminder::toast(app, "ポモドーロをはじめるよ！");
        }
        "pomo-stop" => pomodoro::stop_now(app, &shared),
        "notes-save-clip" => match notes::save_clipboard(app, &shared) {
            Ok(_) => reminder::toast(app, "クリップボードをメモに保存したよ"),
            Err(e) => reminder::toast(app, &e),
        },
        id if id.starts_with("note-item-") => {
            let copied = id["note-item-".len()..]
                .parse()
                .is_ok_and(|n| notes::copy_note(&shared, n));
            reminder::toast(
                app,
                if copied {
                    "コピーしたよ"
                } else {
                    "コピーできなかった"
                },
            );
        }
        "weather-now" => {
            let _ = window.emit("weather-now", ());
        }
        "toggle" => {
            let visible = !window.is_visible().unwrap_or(false);
            if !visible {
                // 出していたふきだしも一緒にしまう
                bubble::dismiss(app);
            }
            let _ = if visible {
                window.show()
            } else {
                window.hide()
            };
            shared.visible.store(visible, Ordering::Relaxed);
            shared.store.update(|s| s.visible = visible);
        }
        "quit" => {
            save_current_position(&window, &shared);
            app.exit(0);
        }
        _ => {
            // 速さ・大きさ・キャラクターはラジオボタンのように 1 つだけチェックする
            if let Some(&(character, _)) = items.characters.iter().find(|(c, _)| c.id() == id) {
                for (c, item) in &items.characters {
                    let _ = item.set_checked(*c == character);
                }
                shared.store.update(|s| s.character = character);
                emit_settings(&window, &shared);
            }
        }
    }
}
