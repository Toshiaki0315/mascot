//! トレイメニューで変更できる設定と、終了後も残す状態（位置・表示）を JSON に保存する

use std::{fs, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};

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
