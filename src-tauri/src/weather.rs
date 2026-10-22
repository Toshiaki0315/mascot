//! 天気の設定（地名と緯度経度、朝に知らせるか）。取得と表示はフロントエンド（Open-Meteo）で行う

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::Shared;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WeatherSettings {
    /// 朝に天気を知らせる
    pub enabled: bool,
    /// 画面に出す地名
    pub city: String,
    pub lat: f64,
    pub lon: f64,
    /// 知らせる時刻（時）。この時刻を過ぎて最初に確認したときに、1 日 1 回知らせる
    pub hour: u8,
    /// 最後に知らせた日（YYYY-MM-DD）
    pub last_date: String,
}

impl Default for WeatherSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            city: String::new(),
            lat: 0.0,
            lon: 0.0,
            hour: 7,
            last_date: String::new(),
        }
    }
}

impl WeatherSettings {
    fn valid(&self) -> bool {
        self.hour < 24
            && (-90.0..=90.0).contains(&self.lat)
            && (-180.0..=180.0).contains(&self.lon)
            && self.city.chars().count() <= 60
            // 有効にするなら、場所が決まっていること
            && (!self.enabled || !self.city.is_empty())
    }
}

#[tauri::command]
pub fn get_weather_settings(shared: State<Arc<Shared>>) -> WeatherSettings {
    shared.store.get().weather
}

#[tauri::command]
pub fn set_weather_settings(
    shared: State<Arc<Shared>>,
    weather: WeatherSettings,
) -> Result<(), String> {
    if !weather.valid() {
        return Err("天気の設定が正しくありません（場所を選んでください）".into());
    }
    shared.store.update(|s| s.weather = weather);
    Ok(())
}

/// 今日の分を知らせたと記録する
#[tauri::command]
pub fn mark_weather_shown(shared: State<Arc<Shared>>, date: String) {
    shared.store.update(|s| s.weather.last_date = date);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_off() {
        let w = WeatherSettings::default();
        assert!(!w.enabled && w.hour == 7 && w.city.is_empty());
    }

    #[test]
    fn enabling_needs_a_place() {
        let mut w = WeatherSettings {
            enabled: true,
            ..WeatherSettings::default()
        };
        assert!(!w.valid());
        w.city = "東京".into();
        w.lat = 35.68;
        w.lon = 139.69;
        assert!(w.valid());
        w.hour = 24;
        assert!(!w.valid());
    }

    #[test]
    fn out_of_range_location_is_rejected() {
        let w = WeatherSettings {
            city: "x".into(),
            lat: 91.0,
            ..WeatherSettings::default()
        };
        assert!(!w.valid());
    }
}
