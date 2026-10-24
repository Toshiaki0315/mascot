import { invoke } from "@tauri-apps/api/core";
import { listen, type Event } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Bubble, hourLine, lineFor, type Occasion } from "./speech";
import { CHARACTERS, renderSprite, type Character } from "./sprite";
import {
  fetchForecastJson,
  localDate,
  parseForecast,
  shouldRetryWeather,
  WEATHER_RETRY_MS,
  weatherMessage,
  type WeatherSettings,
} from "./weather";

type CursorPayload = { x: number; y: number };
type MovePayload = { moving: boolean; dir: -1 | 0 | 1 };
type FlagPayload = { on: boolean };
type SettingsPayload = { chatty: boolean; character: string };
type SayPayload = { kind: "break" };

// ---- Rust 側からの通知の受け取り ----

/// 受け取りの準備（listen）の一覧。すべて片付いたら frontend_ready を呼ぶ
const listening: Promise<unknown>[] = [];

/// Rust 側からの通知を受け取る。準備を待たずに先へ進み、1 つが失敗しても、
/// ほかの受け取りやマウスの処理は動くようにする（以前は 1 つ失敗すると、
/// その後ろで登録するクリックやドラッグの処理まで止まり、マウス操作がまったく効かなくなった）
function on<T>(event: string, handler: (e: Event<T>) => void) {
  listening.push(
    listen<T>(event, (e) => {
      try {
        handler(e);
      } catch (err) {
        console.error(`「${event}」の処理でエラーが起きました`, err);
      }
    }).catch((err) => console.error(`「${event}」を受け取れません`, err)),
  );
}

/// 時刻になったリマインダー（表示待ちの列）
type Announce = { text: string; snooze: boolean };
const pendingReminders: Announce[] = [];
/// いまふきだしに出しているリマインダー（全文。「5 分後」を押したときに使う）
let currentReminder: Announce = { text: "", snooze: false };
/// 大きなふきだし（専用のウィンドウ）を出している間か
let reminderShown = false;
/// 放っておいたふきだしを自動で閉じるまでの時間
const REMIND_MS = 5 * 60_000;
let remindTimer: number | undefined;

const stage = document.querySelector<HTMLElement>("#stage")!;
const mascot = document.querySelector<SVGSVGElement>("#mascot")!;
// ふつうのおしゃべりも、大きなふきだし（専用のウィンドウ）に出す
const bubble = new Bubble(
  (text) => void invoke("show_bubble", { text, snooze: false, chat: true }),
  () => void invoke("hide_bubble"),
);

let character: Character = "tora";
let chatty = true;
renderSprite(mascot, character);

let overMascot = false;
let walking = false;
let flying = false;
let sleeping = false;
let suppressed = false;

function say(occasion: Occasion) {
  if (!chatty || suppressed || reminderShown) return;
  const line = lineFor(occasion, character);
  if (line) bubble.say(line);
}

// リマインダーを 1 件ずつ、大きなふきだし（専用のウィンドウ）に出す。閉じられたら次を出す。
// 全画面アプリの上で隠れている間は出さず、戻ってから出す
function showNextReminder() {
  if (suppressed || reminderShown) return;
  const next = pendingReminders.shift();
  if (next === undefined) return;
  currentReminder = next;
  reminderShown = true;
  bubble.hide();
  rejoice();
  void invoke("show_bubble", { text: next.text, snooze: next.snooze, chat: false });
  remindTimer = window.setTimeout(() => closeReminder(false), REMIND_MS);
}

// ふきだしを閉じて、次のリマインダーがあれば出す。snooze なら 5 分後にもう一度知らせる
function closeReminder(snooze: boolean) {
  if (!reminderShown) return;
  reminderShown = false;
  clearTimeout(remindTimer);
  if (snooze && currentReminder.snooze) {
    // 件数の上限を超えるときは、理由が返るので、ひとことで伝える
    invoke("snooze_reminder", { text: currentReminder.text }).catch((err) => bubble.say(String(err)));
  }
  void invoke("hide_bubble");
  showNextReminder();
}

// 指定座標（ウィンドウ内の CSS ピクセル）がキャラクターの不透明部分か
function hitTest(x: number, y: number): boolean {
  if (suppressed) return false;
  if (x < 0 || y < 0 || x >= window.innerWidth || y >= window.innerHeight) {
    return false;
  }
  const el = document.elementFromPoint(x, y);
  return el instanceof SVGElement && el.classList.contains("hit");
}

async function updateHover(x: number, y: number) {
  const hit = hitTest(x, y);
  if (hit === overMascot) return;
  overMascot = hit;
  mascot.classList.toggle("hover", hit);
  if (hit) cancelEmote();
  // 不透明部分に乗ったらクリックを受け付け、離れたら再び透過する
  await invoke("set_click_through", { ignore: !hit });
}

// ---- 視線 ----

/// キャラクターの大きさの何倍の距離までカーソルを目で追うか
const LOOK_RANGE = 2.5;

function setLook(dir: -1 | 0 | 1) {
  mascot.classList.toggle("look-left", dir < 0);
  mascot.classList.toggle("look-right", dir > 0);
}

function lookAtCursor(x: number, y: number) {
  const rect = mascot.getBoundingClientRect();
  const dx = x - (rect.left + rect.width / 2);
  const dy = y - (rect.top + rect.height / 2);
  const near = Math.hypot(dx, dy) < rect.width * LOOK_RANGE;
  // 真上・真下あたりにいる時は正面を向く
  setLook(near && Math.abs(dx) > rect.width * 0.2 ? (dx < 0 ? -1 : 1) : 0);
}

// ---- 休憩中のしぐさ（あくび・伸び） ----

const EMOTES = ["yawn", "stretch"] as const;
const EMOTE_CHANCE = 0.5;
const EMOTE_DURATION_MS = 1400;
let emoteTimer: number | undefined;

function cancelEmote() {
  clearTimeout(emoteTimer);
  mascot.classList.remove(...EMOTES);
}

function isBusy() {
  return (
    walking ||
    flying ||
    sleeping ||
    suppressed ||
    overMascot ||
    mascot.classList.contains("happy") ||
    mascot.classList.contains("spinning")
  );
}

// 立ち止まったら、ときどき少し間を置いてしぐさをしたり、ひとこと話したりする
function onRest() {
  cancelEmote();
  say("rest");
  if (Math.random() >= EMOTE_CHANCE) return;
  emoteTimer = window.setTimeout(
    () => {
      if (isBusy()) return;
      const emote = EMOTES[Math.floor(Math.random() * EMOTES.length)];
      mascot.classList.add(emote);
      emoteTimer = window.setTimeout(() => mascot.classList.remove(emote), EMOTE_DURATION_MS);
    },
    1000 + Math.random() * 2000,
  );
}

// ---- 時報 ----

let lastHour = new Date().getHours();
window.setInterval(() => {
  const now = new Date();
  if (now.getMinutes() !== 0 || now.getHours() === lastHour) return;
  lastHour = now.getHours();
  if (chatty && !suppressed && !sleeping && !reminderShown) {
    bubble.say(hourLine(lastHour, character));
  }
}, 20_000);

// ---- Rust 側からの通知 ----

// クリックスルー中は WebView にマウスイベントが届かないため、
// Rust 側がポーリングしたカーソル座標を受け取って判定する
on<CursorPayload>("cursor-moved", (e) => {
  updateHover(e.payload.x, e.payload.y);
  if (!walking && !sleeping && !flying) lookAtCursor(e.payload.x, e.payload.y);
});

// ランダム移動に合わせて、歩くアニメーションと視線を切り替える
on<MovePayload>("mascot-move", (e) => {
  const wasWalking = walking;
  walking = e.payload.moving;
  mascot.classList.toggle("walking", walking);
  if (walking) {
    cancelEmote();
    setLook(e.payload.dir);
  } else {
    setLook(0);
    if (wasWalking) onRest();
  }
});

// 投げられた・落ちている・窓へ飛び乗っている
on<FlagPayload>("mascot-fly", (e) => {
  const wasFlying = flying;
  flying = e.payload.on;
  mascot.classList.toggle("flying", flying);
  if (flying) {
    cancelEmote();
    setLook(0);
  } else if (wasFlying) {
    say("landed");
  }
});

on<FlagPayload>("mascot-sleep", (e) => {
  const wasSleeping = sleeping;
  sleeping = e.payload.on;
  mascot.classList.toggle("sleeping", sleeping);
  cancelEmote();
  setLook(0);
  if (sleeping) bubble.hide();
  else if (wasSleeping) say("wake");
});

on<FlagPayload>("mascot-suppress", (e) => {
  suppressed = e.payload.on;
  stage.classList.toggle("suppressed", suppressed);
  if (suppressed) {
    // 姿が見えなくなるので、出しかけのリマインダーは通知センターへ回す
    for (const item of [reminderShown ? currentReminder : null, ...pendingReminders.splice(0)]) {
      if (item) void invoke("notify_reminder", { text: item.text });
    }
    if (reminderShown) {
      reminderShown = false;
      clearTimeout(remindTimer);
      void invoke("hide_bubble");
    }
    // Rust 側でクリックスルーに戻しているので、こちらの状態も合わせる
    overMascot = false;
    mascot.classList.remove("hover");
    cancelEmote();
    bubble.hide();
  }
});

/// Rust 側の設定（キャラクターとおしゃべり）を、絵と動きに反映する。
/// 通知（イベント）で届くほか、起動直後の返事と、定期的な見比べでも呼ぶ
function applySettings(payload: SettingsPayload) {
  chatty = payload.chatty;
  if (!chatty) bubble.hide();
  const next = CHARACTERS.find((c) => c === payload.character) ?? "tora";
  if (next !== character) {
    character = next;
    renderSprite(mascot, character);
  }
}

on<SettingsPayload>("mascot-settings", (e) => applySettings(e.payload));

on<Announce>("mascot-reminder", (e) => {
  // 全画面アプリの上などで姿が見えない間は、通知センターに出す
  if (suppressed) {
    void invoke("notify_reminder", { text: e.payload.text });
    return;
  }
  pendingReminders.push(e.payload);
  showNextReminder();
});

// コピーしたよ、など短いひとこと（おしゃべりの設定に関係なく出す）
on<string>("mascot-toast", (e) => {
  if (!suppressed && !sleeping && !reminderShown) bubble.say(e.payload);
});

// ---- 天気 ----

/// 今日の天気を知らせる。`force` はメニューからの「今日の天気」（時刻や 1 日 1 回の制限を無視する）
/// 取得の最中か（やり直しと 10 分ごとの確認が重ならないようにする）
let weatherChecking = false;
/// 続けて失敗した回数（成功したら 0 に戻す）
let weatherRetries = 0;

async function checkWeather(force: boolean) {
  if (weatherChecking) return;
  const settings = await invoke<WeatherSettings>("get_weather_settings");
  const today = localDate(new Date());
  if (!force && (!settings.enabled || settings.last_date === today || new Date().getHours() < settings.hour)) {
    return;
  }
  if (!settings.city) {
    if (force) bubble.say("場所を決めてね（リマインダー画面の「天気」）");
    return;
  }
  weatherChecking = true;
  try {
    const forecast = parseForecast(await fetchForecastJson(settings.lat, settings.lon));
    pendingReminders.push({ text: weatherMessage(settings.city, forecast), snooze: false });
    showNextReminder();
    weatherRetries = 0;
    if (!force) void invoke("mark_weather_shown", { date: today });
  } catch {
    if (force) {
      bubble.say("天気を取得できなかったよ");
    } else if (shouldRetryWeather(weatherRetries)) {
      // ネットワークがまだ使えないのかもしれないので、少し待ってやり直す
      weatherRetries += 1;
      window.setTimeout(() => void checkWeather(false), WEATHER_RETRY_MS);
    }
  } finally {
    weatherChecking = false;
  }
}

on("weather-now", () => void checkWeather(true));

// タイマーの最後の秒読み（10 → 1）。眠っていても出す
on<string>("mascot-countdown", (e) => {
  if (!suppressed && !reminderShown) bubble.say(e.payload);
});

on<SayPayload>("mascot-say", (e) => {
  say(e.payload.kind);
});

// 朝の天気：起動直後と、そのあと 10 分ごとに確認する（知らせるのは 1 日 1 回）
void checkWeather(false);
window.setInterval(() => void checkWeather(false), 10 * 60_000);

// ---- クリック・ダブルクリック・ドラッグ ----

// 押してそのまま動かしたらドラッグ、その場で離したらクリックとして扱う。
// ドラッグして勢いよく離すと、Rust 側がその速さで投げる
const DRAG_THRESHOLD = 4;
const HAPPY_DURATION_MS = 1000;
const DOUBLE_CLICK_WAIT_MS = 250;
const SPIN_DURATION_MS = 800;
let pressAt: { x: number; y: number } | null = null;
let happyTimer: number | undefined;
let clickTimer: number | undefined;
let spinTimer: number | undefined;

function rejoice() {
  // 連打されたらアニメーションを最初からやり直す
  cancelEmote();
  mascot.classList.remove("happy");
  void mascot.getBoundingClientRect();
  mascot.classList.add("happy");
  clearTimeout(happyTimer);
  happyTimer = window.setTimeout(() => mascot.classList.remove("happy"), HAPPY_DURATION_MS);
  say("click");
}

function spin() {
  cancelEmote();
  mascot.classList.remove("happy", "spinning");
  void mascot.getBoundingClientRect();
  mascot.classList.add("spinning");
  clearTimeout(spinTimer);
  spinTimer = window.setTimeout(() => mascot.classList.remove("spinning"), SPIN_DURATION_MS);
  say("spin");
}

// 大きなふきだしのボタンやクリック（専用のウィンドウから Rust 経由で届く）。
// マスコットを隠したときなど、Rust 側で閉じたときも "close" で届く
on<string>("reminder-action", (e) => {
  // リマインダーを出していなければ、ふつうのおしゃべりのクリック
  if (reminderShown) closeReminder(e.payload === "snooze");
  else bubble.hide();
});

// 右クリックで、トレイと同じメニューをその場に出す
window.addEventListener("contextmenu", (e) => {
  e.preventDefault();
  if (!overMascot) return;
  pressAt = null;
  void invoke("show_context_menu");
});

mascot.addEventListener("mousedown", (e) => {
  if (e.button === 0 && overMascot) {
    pressAt = { x: e.clientX, y: e.clientY };
  }
});

window.addEventListener("mousemove", (e) => {
  if (!pressAt) return;
  if (Math.hypot(e.clientX - pressAt.x, e.clientY - pressAt.y) >= DRAG_THRESHOLD) {
    pressAt = null;
    bubble.hide();
    // ドラッグで動かした先から歩き出せるよう、Rust 側にドラッグの始まりを知らせる。
    // macOS のドラッグはマウスのイベントの最中に始める必要があるので、返事は待たない
    void invoke("start_drag");
    getCurrentWindow().startDragging();
  }
});

// シングルクリックはダブルクリックでないと分かるまで少し待ってから喜ぶ
window.addEventListener("mouseup", (e) => {
  if (pressAt && e.button === 0) {
    clearTimeout(clickTimer);
    if (reminderShown) {
      // キャラクターをクリックしても、リマインダーを閉じられる
      closeReminder(false);
    } else if (e.detail >= 2) {
      spin();
    } else {
      clickTimer = window.setTimeout(rejoice, DOUBLE_CLICK_WAIT_MS);
    }
  }
  pressAt = null;
});

// 受け取りの準備がすべて片付いたら（失敗したものがあっても）、
// 起動直後に送られて取りこぼした状態を送り直してもらう
// 返事にも、いまの設定が入っている（通知を取りこぼしても、これで追いつく）
void Promise.all(listening).then(() =>
  invoke<SettingsPayload>("frontend_ready").then(applySettings),
);

/// 念のため、60 秒に 1 回、Rust 側の設定と見比べる（違っていれば描き直す。同じなら何もしない）
const SETTINGS_CHECK_MS = 60_000;
window.setInterval(() => {
  invoke<SettingsPayload>("get_mascot_settings")
    .then(applySettings)
    .catch((err) => console.error("設定を取得できません", err));
}, SETTINGS_CHECK_MS);
