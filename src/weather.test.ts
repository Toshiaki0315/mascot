import assert from "node:assert/strict";
import { test } from "node:test";
import {
  describeWeather,
  localDate,
  shouldRetryWeather,
  WEATHER_MAX_RETRIES,
  WEATHER_RETRY_MS,
  parseForecast,
  parsePlaces,
  weatherMessage,
} from "./weather.ts";

const sample = {
  daily: {
    weather_code: [61],
    temperature_2m_max: [24.4],
    temperature_2m_min: [15.6],
    precipitation_probability_max: [70],
  },
};

test("天気コードを日本語にする", () => {
  assert.equal(describeWeather(0), "快晴");
  assert.equal(describeWeather(2), "薄曇り");
  assert.equal(describeWeather(63), "雨");
  assert.equal(describeWeather(81), "にわか雨");
  assert.equal(describeWeather(73), "雪");
  assert.equal(describeWeather(95), "雷雨");
  assert.equal(describeWeather(999), "不明");
});

test("予報の JSON から今日の分を取り出し、気温は四捨五入する", () => {
  assert.deepEqual(parseForecast(sample), { code: 61, max: 24, min: 16, rain: 70 });
});

test("降水確率がなくても読める", () => {
  const json = { daily: { weather_code: [0], temperature_2m_max: [10], temperature_2m_min: [2] } };
  assert.equal(parseForecast(json).rain, 0);
});

test("形式が違う JSON は例外にする", () => {
  assert.throws(() => parseForecast({}));
  assert.throws(() => parseForecast(null));
});

test("ふきだしの文は短くまとまる（ふつうの地名なら 24 文字以内）", () => {
  const message = weatherMessage("東京", parseForecast(sample));
  assert.equal(message, "東京：雨 24/16℃ 雨70%");
  assert.ok([...weatherMessage("鹿児島", { code: 95, max: -10, min: -20, rain: 100 })].length <= 24);
});

test("ローカルの日付は YYYY-MM-DD", () => {
  assert.equal(localDate(new Date(2026, 0, 5, 23, 59)), "2026-01-05");
});

test("場所の候補を取り出す（不完全なものは捨てる）", () => {
  const json = {
    results: [
      { name: "東京", admin1: "東京都", country: "日本", latitude: 35.68, longitude: 139.69 },
      { name: "壊れた" },
    ],
  };
  assert.deepEqual(parsePlaces(json), [
    { name: "東京", detail: "東京都・日本", lat: 35.68, lon: 139.69 },
  ]);
  assert.deepEqual(parsePlaces({}), []);
});

test("取得に失敗したときは、1 分あけて、3 回までやり直す", () => {
  assert.equal(WEATHER_RETRY_MS, 60_000);
  assert.equal(WEATHER_MAX_RETRIES, 3);
  assert.equal(shouldRetryWeather(0), true);
  assert.equal(shouldRetryWeather(2), true);
  assert.equal(shouldRetryWeather(3), false);
  assert.equal(shouldRetryWeather(10), false);
});
