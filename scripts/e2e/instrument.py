# e2e テスト用のアプリにだけ入れる変更（作業用のコピーに対して実行する。リポジトリには入れない）
#   - 識別子を分けて、普段使っているアプリ・設定とぶつからないようにする
#   - 起動 10 秒後に、天気と同じ種類のふきだし（リマインダー）を出す
import json

p = "src-tauri/tauri.conf.json"
d = json.load(open(p))
d["identifier"] = "jp.classmethod.desktop-mascot.e2e"
json.dump(d, open(p, "w"), indent=2, ensure_ascii=False)

p = "src-tauri/src/lib.rs"
s = open(p).read()
anchor = "            spawn_cursor_watcher(app.handle().clone(), shared.clone());"
assert anchor in s, "lib.rs の目印が見つかりません（e2e/instrument.py を直してください）"
s = s.replace(anchor, """            {
                let handle = app.handle().clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_secs(10));
                    if let Some(w) = handle.get_webview_window(MAIN_WINDOW) {
                        let _ = w.emit("mascot-reminder", serde_json::json!({"text": "e2e: 曇り 19/14°C", "snooze": false}));
                    }
                });
            }
""" + anchor, 1)
open(p, "w").write(s)
