import { invoke } from "@tauri-apps/api/core";
import {
  fetchForecastJson,
  parseForecast,
  searchPlaces,
  weatherMessage,
  type Place,
  type WeatherSettings,
} from "./weather";

const form = document.querySelector<HTMLFormElement>("#weather-form")!;
const enabledBox = document.querySelector<HTMLInputElement>("#weather-enabled")!;
const cityInput = document.querySelector<HTMLInputElement>("#weather-city")!;
const searchButton = document.querySelector<HTMLButtonElement>("#weather-search")!;
const placesEl = document.querySelector<HTMLUListElement>("#weather-places")!;
const placeEl = document.querySelector<HTMLElement>("#weather-place")!;
const hourSelect = document.querySelector<HTMLSelectElement>("#weather-hour")!;
const errorEl = document.querySelector<HTMLElement>("#weather-error")!;
const nowButton = document.querySelector<HTMLButtonElement>("#weather-now")!;
const resultEl = document.querySelector<HTMLElement>("#weather-result")!;

let settings: WeatherSettings = await invoke<WeatherSettings>("get_weather_settings");

for (let h = 4; h <= 12; h++) {
  const option = document.createElement("option");
  option.value = String(h);
  option.textContent = `${h} 時`;
  hourSelect.append(option);
}

function show() {
  enabledBox.checked = settings.enabled;
  hourSelect.value = String(settings.hour);
  placeEl.textContent = settings.city ? `場所：${settings.city}` : "場所はまだ決まっていません";
}

function choose(place: Place) {
  settings = { ...settings, city: place.name, lat: place.lat, lon: place.lon };
  placesEl.replaceChildren();
  cityInput.value = "";
  show();
}

searchButton.addEventListener("click", async () => {
  errorEl.textContent = "";
  const name = cityInput.value.trim();
  if (!name) return;
  try {
    const places = await searchPlaces(name);
    if (places.length === 0) errorEl.textContent = "見つかりませんでした";
    placesEl.replaceChildren(
      ...places.map((p) => {
        const li = document.createElement("li");
        const text = document.createElement("span");
        text.className = "text";
        text.textContent = p.name;
        const detail = document.createElement("span");
        detail.className = "when";
        detail.textContent = p.detail;
        const pick = document.createElement("button");
        pick.type = "button";
        pick.className = "edit";
        pick.textContent = "ここにする";
        pick.addEventListener("click", () => choose(p));
        const wrap = document.createElement("span");
        wrap.className = "row-buttons";
        wrap.append(pick);
        li.append(text, detail, wrap);
        return li;
      }),
    );
  } catch (err) {
    errorEl.textContent = String(err);
  }
});

// Enter キーで保存せず、場所を探す
cityInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    searchButton.click();
  }
});

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  errorEl.textContent = "";
  const next = { ...settings, enabled: enabledBox.checked, hour: Number(hourSelect.value) };
  try {
    await invoke("set_weather_settings", { weather: next });
    settings = next;
    errorEl.textContent = "";
    resultEl.textContent = "保存しました";
  } catch (err) {
    errorEl.textContent = String(err);
  }
});

nowButton.addEventListener("click", async () => {
  errorEl.textContent = "";
  resultEl.textContent = "";
  if (!settings.city) {
    errorEl.textContent = "先に場所を決めてください";
    return;
  }
  try {
    resultEl.textContent = weatherMessage(settings.city, parseForecast(await fetchForecastJson(settings.lat, settings.lon)));
  } catch (err) {
    errorEl.textContent = String(err);
  }
});

show();
