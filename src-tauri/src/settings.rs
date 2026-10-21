//! トレイメニューで変更できる設定と、終了後も残す状態（位置・表示）を JSON に保存する

use std::{fs, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};

use crate::reminder::Reminder;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speed {
    Slow,
    Normal,
    Fast,
}

impl Speed {
    pub const ALL: [Speed; 3] = [Speed::Slow, Speed::Normal, Speed::Fast];

    /// 歩く速さ（論理ピクセル/秒、等倍のとき）
    pub fn px_per_sec(self) -> f64 {
        match self {
            Speed::Slow => 30.0,
            Speed::Normal => 60.0,
            Speed::Fast => 120.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Speed::Slow => "ゆっくり",
            Speed::Normal => "ふつう",
            Speed::Fast => "はやい",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Speed::Slow => "speed-slow",
            Speed::Normal => "speed-normal",
            Speed::Fast => "speed-fast",
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
}

impl Character {
    pub const ALL: [Character; 6] = [
        Character::Tora,
        Character::Kuro,
        Character::Shiro,
        Character::Mike,
        Character::Inu,
        Character::Kuranyan,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Character::Tora => "茶トラ",
            Character::Kuro => "黒ネコ",
            Character::Shiro => "白ネコ",
            Character::Mike => "三毛ネコ",
            Character::Inu => "いぬ",
            Character::Kuranyan => "くらにゃんぽい",
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
    pub reminder_sound: bool,
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
        }
    }
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
        if let Some(dir) = self.path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(&*settings) {
            let _ = fs::write(&self.path, json);
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
            s.reminders.push(Reminder {
                id: 1,
                text: "会議".into(),
                at: 1234,
                repeat: Repeat::Daily,
            })
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
        for c in Character::ALL {
            assert!(c.id().starts_with("char-"));
        }
    }
}
