// 天気：Open-Meteo（キー不要）から今日の予報を取り、ふきだしに出す文にする

export type WeatherSettings = {
  enabled: boolean;
  city: string;
  lat: number;
  lon: number;
  hour: number;
  last_date: string;
};

export type Forecast = { code: number; max: number; min: number; rain: number };

export type Place = { name: string; detail: string; lat: number; lon: number };

/// WMO の天気コードを、短い日本語にする
export function describeWeather(code: number): string {
  if (code === 0) return "快晴";
  if (code === 1) return "晴れ";
  if (code === 2) return "薄曇り";
  if (code === 3) return "曇り";
  if (code === 45 || code === 48) return "霧";
  if (code >= 51 && code <= 57) return "霧雨";
  if (code === 66 || code === 67) return "凍る雨";
  if (code >= 61 && code <= 65) return "雨";
  if (code >= 71 && code <= 77) return "雪";
  if (code >= 80 && code <= 82) return "にわか雨";
  if (code === 85 || code === 86) return "にわか雪";
  if (code >= 95 && code <= 99) return "雷雨";
  return "不明";
}

/// 予報の JSON（daily の配列の先頭が今日）から、今日の分を取り出す
export function parseForecast(json: unknown): Forecast {
  const daily = (json as { daily?: Record<string, number[] | undefined> })?.daily;
  const code = daily?.weather_code?.[0];
  const max = daily?.temperature_2m_max?.[0];
  const min = daily?.temperature_2m_min?.[0];
  const rain = daily?.precipitation_probability_max?.[0];
  if (code === undefined || max === undefined || min === undefined) {
    throw new Error("天気の形式が想定と違います");
  }
  return { code, max: Math.round(max), min: Math.round(min), rain: Math.round(rain ?? 0) };
}

/// ふきだしに出す文（30 文字以内に収まる長さ）
export function weatherMessage(city: string, f: Forecast): string {
  return `${city}：${describeWeather(f.code)} ${f.max}/${f.min}℃ 雨${f.rain}%`;
}

/// YYYY-MM-DD（ローカルの日付）
export function localDate(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export async function fetchForecastJson(lat: number, lon: number): Promise<unknown> {
  const url =
    "https://api.open-meteo.com/v1/forecast" +
    `?latitude=${lat}&longitude=${lon}` +
    "&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max" +
    "&timezone=auto&forecast_days=1";
  const res = await fetch(url);
  if (!res.ok) throw new Error(`天気を取得できませんでした（${res.status}）`);
  return res.json();
}

/// 地名から場所の候補を探す（Open-Meteo のジオコーディング）
export function parsePlaces(json: unknown): Place[] {
  const results = (json as { results?: Record<string, unknown>[] })?.results ?? [];
  return results.flatMap((r) =>
    typeof r.name === "string" && typeof r.latitude === "number" && typeof r.longitude === "number"
      ? [
          {
            name: r.name,
            detail: [r.admin1, r.country].filter((x) => typeof x === "string" && x).join("・"),
            lat: r.latitude,
            lon: r.longitude,
          },
        ]
      : [],
  );
}

export async function searchPlaces(name: string): Promise<Place[]> {
  const url =
    "https://geocoding-api.open-meteo.com/v1/search" +
    `?name=${encodeURIComponent(name)}&count=5&language=ja&format=json`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`場所を探せませんでした（${res.status}）`);
  return parsePlaces(await res.json());
}
