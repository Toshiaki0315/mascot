//! トレイメニューで変更できる設定と、終了後も残す状態（位置・表示）を JSON に保存する

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

use crate::{
    notes::Note,
    pomodoro::{Pomodoro, PomodoroCfg},
    reminder::Reminder,
    weather::WeatherSettings,
};

/// タイマーの最後の秒読み（10 → 1）をふきだしに出すか
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CountdownMode {
    /// タイマーごとに、出すかをランダム（半々）に決める
    #[default]
    Random,
    Always,
    Never,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speed {
    Slow,
    Normal,
    Fast,
}

impl Speed {
    /// 歩く速さ（論理ピクセル/秒、等倍のとき）
    pub fn px_per_sec(self) -> f64 {
        match self {
            Speed::Slow => 30.0,
            Speed::Normal => 60.0,
            Speed::Fast => 120.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Character {
    Tora,
    Kuro,
    Shiro,
    Mike,
    Inu,
    Kuranyan,
    /// 自分で選んだ画像（1 つだけ。custom.rs）
    Custom,
}

impl Character {
    pub const ALL: [Character; 7] = [
        Character::Tora,
        Character::Kuro,
        Character::Shiro,
        Character::Mike,
        Character::Inu,
        Character::Kuranyan,
        Character::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Character::Tora => "茶トラ",
            Character::Kuro => "黒ネコ",
            Character::Shiro => "白ネコ",
            Character::Mike => "三毛ネコ",
            Character::Inu => "いぬ",
            Character::Kuranyan => "くらにゃんぽい",
            Character::Custom => "じぶんの絵",
        }
    }

    /// メニュー項目の ID。フロントエンドにはこれから "char-" を除いた名前で伝える
    pub fn id(self) -> &'static str {
        match self {
            Character::Tora => "char-tora",
            Character::Kuro => "char-kuro",
            Character::Shiro => "char-shiro",
            Character::Mike => "char-mike",
            Character::Inu => "char-inu",
            Character::Kuranyan => "char-kuranyan",
            Character::Custom => "char-custom",
        }
    }

    pub fn name(self) -> &'static str {
        &self.id()["char-".len()..]
    }
}

/// 選べる大きさ（等倍に対する倍率）
pub const SCALES: [f64; 3] = [0.5, 1.0, 2.0];

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// ウィンドウ左上の位置（macOS のポイント単位）
    pub position: Option<(f64, f64)>,
    pub visible: bool,
    pub walking: bool,
    pub speed: Speed,
    pub scale: f64,
    pub hide_on_fullscreen: bool,
    /// ふきだしでおしゃべりする
    pub chatty: bool,
    pub character: Character,
    pub reminders: Vec<Reminder>,
    /// リマインダーの時刻に音を鳴らす
    /// 時刻になったときに音を鳴らすか（リマインダー / タイマー / ポモドーロ）
    pub reminder_sound: bool,
    pub timer_sound: bool,
    pub pomodoro_sound: bool,
    /// メニューバーに、動いているタイマー / ポモドーロの残り時間を出すか
    pub menubar_timer: bool,
    pub menubar_pomodoro: bool,
    /// タイマーの最後の秒読みをふきだしに出すか
    pub countdown_mode: CountdownMode,
    pub notes: Vec<Note>,
    /// 動いているポモドーロ（なければ None）
    pub pomodoro: Option<Pomodoro>,
    pub pomodoro_cfg: PomodoroCfg,
    pub weather: WeatherSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            position: None,
            visible: true,
            walking: true,
            speed: Speed::Normal,
            scale: 1.0,
            hide_on_fullscreen: true,
            chatty: true,
            character: Character::Tora,
            reminders: Vec::new(),
            reminder_sound: true,
            timer_sound: true,
            pomodoro_sound: true,
            menubar_timer: true,
            menubar_pomodoro: true,
            countdown_mode: CountdownMode::Random,
            notes: Vec::new(),
            pomodoro: None,
            pomodoro_cfg: PomodoroCfg::default(),
            weather: WeatherSettings::default(),
        }
    }
}

/// 設定を JSON にして保存する。いったん別のファイルに書いてから置き換えるので、
/// 書いている途中でアプリが終わっても、前の設定ファイルが壊れない
fn write_atomically(path: &Path, settings: &Settings) -> Result<(), String> {
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

pub struct Store {
    path: PathBuf,
    settings: Mutex<Settings>,
}

impl Store {
    pub fn load(path: PathBuf) -> Self {
        let mut settings: Settings = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if !SCALES.contains(&settings.scale) {
            settings.scale = 1.0;
        }
        Self {
            path,
            settings: Mutex::new(settings),
        }
    }

    pub fn get(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    /// 設定を変更してファイルに書き出す
    pub fn update(&self, f: impl FnOnce(&mut Settings)) {
        let mut settings = self.settings.lock().unwrap();
        f(&mut settings);
        // 保存できなくても、アプリは動き続ける。ただ、次の起動で設定が戻ってしまうので、理由を残す
        if let Err(e) = write_atomically(&self.path, &settings) {
            eprintln!("設定を保存できませんでした（{}）: {e}", self.path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("desktop-mascot-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn missing_file_gives_defaults() {
        let store = Store::load(temp_path("missing.json"));
        let s = store.get();
        assert!(s.visible && s.walking && s.chatty && s.hide_on_fullscreen);
        assert_eq!(s.scale, 1.0);
        assert!(s.speed == Speed::Normal && s.character == Character::Tora);
    }

    #[test]
    fn broken_file_gives_defaults() {
        let path = temp_path("broken.json");
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(Store::load(path).get().scale, 1.0);
    }

    #[test]
    fn older_file_keeps_values_and_fills_new_fields() {
        // v0.2.0 の設定ファイル（scale が整数、chatty / character がない）
        let path = temp_path("old.json");
        fs::write(
            &path,
            r#"{"position":[10.0,20.0],"visible":false,"walking":true,"speed":"fast","scale":2,"hide_on_fullscreen":false}"#,
        )
        .unwrap();
        let s = Store::load(path).get();
        assert_eq!(s.position, Some((10.0, 20.0)));
        assert!(!s.visible && !s.hide_on_fullscreen);
        assert!(s.speed == Speed::Fast && s.scale == 2.0);
        assert!(s.chatty && s.character == Character::Tora);
    }

    #[test]
    fn reminders_are_saved_and_reloaded() {
        use crate::reminder::Repeat;
        let path = temp_path("reminders.json");
        let _ = fs::remove_file(&path);
        let store = Store::load(path.clone());
        store.update(|s| {
            s.reminders
                .push(Reminder::new(1, "会議".into(), 1234, Repeat::Daily, 0))
        });
        let s = Store::load(path).get();
        assert_eq!(s.reminders.len(), 1);
        assert!(s.reminders[0].text == "会議" && s.reminders[0].repeat == Repeat::Daily);
    }

    #[test]
    fn unknown_scale_falls_back_to_1x() {
        let path = temp_path("scale.json");
        fs::write(&path, r#"{"scale":3.5}"#).unwrap();
        assert_eq!(Store::load(path).get().scale, 1.0);
    }

    #[test]
    fn update_is_saved_and_reloaded() {
        let path = temp_path("roundtrip.json");
        let _ = fs::remove_file(&path);
        let store = Store::load(path.clone());
        store.update(|s| {
            s.character = Character::Inu;
            s.scale = 0.5;
        });
        let s = Store::load(path).get();
        assert!(s.character == Character::Inu && s.scale == 0.5);
    }

    #[test]
    fn character_names() {
        assert_eq!(Character::Mike.name(), "mike");
        assert_eq!(Character::Kuranyan.name(), "kuranyan");
        assert_eq!(Character::Custom.name(), "custom");
        for c in Character::ALL {
            assert!(c.id().starts_with("char-"));
        }
    }

    #[test]
    fn countdown_mode_defaults_to_random_and_roundtrips() {
        assert_eq!(Settings::default().countdown_mode, CountdownMode::Random);
        // 古い設定ファイルには、秒読みの設定がない
        let s: Settings = serde_json::from_str(r#"{"scale":1}"#).unwrap();
        assert_eq!(s.countdown_mode, CountdownMode::Random);
        let s: Settings = serde_json::from_str(r#"{"countdown_mode":"never"}"#).unwrap();
        assert_eq!(s.countdown_mode, CountdownMode::Never);
    }

    #[test]
    fn saving_leaves_no_temporary_file_and_replaces_the_old_one() {
        let path = temp_path("atomic.json");
        let _ = fs::remove_file(&path);
        let store = Store::load(path.clone());
        store.update(|s| s.scale = 2.0);
        store.update(|s| s.scale = 0.5);
        assert!(!path.with_extension("json.tmp").exists());
        assert_eq!(Store::load(path).get().scale, 0.5);
    }

    #[test]
    fn a_failed_save_keeps_the_settings_in_memory_and_does_not_panic() {
        // 親が「ファイル」なので、ディレクトリを作れず、保存に失敗する
        let blocker = temp_path("blocker");
        fs::write(&blocker, "x").unwrap();
        let store = Store::load(blocker.join("settings.json"));
        store.update(|s| s.walking = false);
        assert!(!store.get().walking);
    }
}
