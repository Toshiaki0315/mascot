// 本体の画面を、メニューバーと Dock を隠した枠なしのウィンドウで覆う（プレゼンテーションの再現）
// 使い方: present <秒数>
import AppKit

let secs = Double(CommandLine.arguments.dropFirst().first ?? "") ?? 6
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let window = NSWindow(
  contentRect: NSScreen.screens[0].frame, styleMask: .borderless, backing: .buffered, defer: false)
window.backgroundColor = NSColor(white: 0.85, alpha: 1)
window.makeKeyAndOrderFront(nil)
app.activate(ignoringOtherApps: true)
app.presentationOptions = [.hideMenuBar, .hideDock]
DispatchQueue.main.asyncAfter(deadline: .now() + secs) { exit(0) }
app.run()
