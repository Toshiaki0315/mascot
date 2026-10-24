// テスト用アプリの、画面に出ているウィンドウを調べる。
// 普段使っているアプリも同じ名前（Desktop Mascot）なので、プロセス番号で見分ける
//   windows <pid> list            … 一覧を出す（マスコットは 148×206、それ以外はふきだし）
//   windows <pid> watch <秒数>    … 20ms ごとに調べ、「ふきだしだけが出ている」回数などを数える
import CoreGraphics
import Foundation

var args = Array(CommandLine.arguments.dropFirst())
let pid = Int(args.removeFirst()) ?? -1

func ours() -> [CGSize] {
  let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
  return list
    .filter { ($0["kCGWindowOwnerPID"] as? Int) == pid && ($0["kCGWindowLayer"] as! Int) == 5 }
    .map { CGRect(dictionaryRepresentation: $0["kCGWindowBounds"] as! CFDictionary)!.size }
}
func isMascot(_ s: CGSize) -> Bool { s.width == 148 && s.height == 206 }

if args.first == "watch" {
  let secs = Double(args.dropFirst().first ?? "") ?? 6
  let start = Date()
  var bubbleOnly = 0, both = 0, none = 0
  while Date().timeIntervalSince(start) < secs {
    let sizes = ours()
    let mascot = sizes.contains(where: isMascot)
    let bubble = sizes.contains { !isMascot($0) }
    if bubble && !mascot { bubbleOnly += 1 } else if bubble && mascot { both += 1 } else if !bubble && !mascot { none += 1 }
    usleep(20_000)
  }
  print("bubbleOnly=\(bubbleOnly) both=\(both) none=\(none)")
} else {
  let sizes = ours()
  print("mascot=\(sizes.contains(where: isMascot) ? 1 : 0) bubble=\(sizes.contains { !isMascot($0) } ? 1 : 0)")
}
