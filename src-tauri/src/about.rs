//! トレイメニューの「Desktop Mascot について」で開く、macOS 標準の情報パネル

use objc2::{
    runtime::{AnyObject, NSObjectProtocol},
    sel, AnyThread, ClassType, MainThreadMarker,
};
use objc2_app_kit::{
    NSAboutPanelOptionApplicationIcon, NSAboutPanelOptionApplicationName,
    NSAboutPanelOptionApplicationVersion, NSAboutPanelOptionCredits, NSAboutPanelOptionVersion,
    NSApplication, NSFloatingWindowLevel, NSImage, NSPanel,
};
use objc2_foundation::{NSAttributedString, NSData, NSDictionary, NSString};

const COPYRIGHT: &str = "© 2026 NOMURA Toshiaki";
const CREDITS: &str = "デスクトップを歩き回るドット絵のマスコット\n\
                       https://github.com/Toshiaki0315/mascot";
/// .app に入れずに動かした時も同じアイコンになるよう、アイコンを埋め込んでおく
const ICON_PNG: &[u8] = include_bytes!("../icons/128x128@2x.png");
/// ビルドしたコミット（build.rs が埋め込む）
const BUILD: &str = env!("MASCOT_GIT_HASH");

/// このアプリを前面に出す。メインスレッドから呼ぶこと。
/// Dock に出ない常駐アプリは前面に出ていないので、そのままだとウィンドウがほかの窓の後ろに開く。
/// macOS 14 以降は activate、それより前は activateIgnoringOtherApps で前面に出す
pub fn activate_app() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    if app.respondsToSelector(sel!(activate)) {
        app.activate();
    } else {
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
    }
}

/// 情報パネルを開く。メインスレッド（メニューのイベント）から呼ぶこと
pub fn show(name: &str, version: &str) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    activate_app();

    let name = NSString::from_str(name);
    let version = NSString::from_str(version);
    let build = NSString::from_str(BUILD);
    let copyright = NSString::from_str(COPYRIGHT);
    let credits = NSAttributedString::from_nsstring(&NSString::from_str(CREDITS));
    // 著作権表示のキーは AppKit に定数がないので文字列で指定する
    let copyright_key = NSString::from_str("Copyright");

    let mut keys: Vec<&NSString> = unsafe {
        vec![
            NSAboutPanelOptionApplicationName,
            NSAboutPanelOptionApplicationVersion,
            NSAboutPanelOptionVersion,
            NSAboutPanelOptionCredits,
            &copyright_key,
        ]
    };
    let mut values: Vec<&AnyObject> = vec![&name, &version, &build, &credits, &copyright];
    let icon = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(ICON_PNG));
    if let Some(icon) = &icon {
        keys.push(unsafe { NSAboutPanelOptionApplicationIcon });
        values.push(icon);
    }
    let options = NSDictionary::from_slices(&keys, &values);
    unsafe { app.orderFrontStandardAboutPanelWithOptions(&options) };

    // それでも前面に出られないことがあるので、パネル（マスコットのウィンドウ以外の NSPanel）を
    // 「常に手前」のレベルに上げて前に出す
    for window in app.windows() {
        if window.isKindOfClass(NSPanel::class()) {
            window.setLevel(NSFloatingWindowLevel);
            window.orderFrontRegardless();
            window.makeKeyWindow();
        }
    }
}
