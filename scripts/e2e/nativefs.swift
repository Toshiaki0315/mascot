// 本体の画面で、macOS 本来の全画面表示（専用の Space）に入り、指定秒数で戻る
// 使い方: nativefs <秒数>
import AppKit

let secs = Double(CommandLine.arguments.dropFirst().first ?? "") ?? 4
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let screen = NSScreen.screens[0]
let window = NSWindow(
  contentRect: NSRect(x: screen.frame.minX + 200, y: screen.frame.minY + 200, width: 600, height: 400),
  styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false, screen: screen)
window.title = "e2e fullscreen"
window.collectionBehavior = [.fullScreenPrimary]
window.makeKeyAndOrderFront(nil)
app.activate(ignoringOtherApps: true)
DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { window.toggleFullScreen(nil) }
DispatchQueue.main.asyncAfter(deadline: .now() + secs) { window.toggleFullScreen(nil) }
DispatchQueue.main.asyncAfter(deadline: .now() + secs + 1.5) { exit(0) }
app.run()
