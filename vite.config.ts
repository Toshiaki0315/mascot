import { defineConfig } from "vite";

// Tauri は固定ポートの dev サーバーを前提とする
export default defineConfig({
  clearScreen: false,
  // マスコット本体のほかに、リマインダーの設定ウィンドウのページがある
  build: {
    rollupOptions: {
      input: { main: "index.html", reminders: "reminders.html" },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
