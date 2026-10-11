//! macOS implementation using AppKit through the `objc2` crates.
//!
//! Reading pasteboard contents is the only place clipboard data enters the
//! process. Callers must never log what these functions return.

use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSEvent, NSPasteboard, NSPasteboardTypeString,
    NSScreen, NSWorkspace,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSString};

use super::{ActiveScreen, ScreenRect, SourceApp, WriteReceipt};

/// Current change count of the general pasteboard. Reading it does not read content.
pub fn change_count() -> i64 {
    NSPasteboard::generalPasteboard().changeCount() as i64
}

/// Type identifiers present on the pasteboard, without duplicates.
/// Combines the pasteboard level list and each item's list, because a type that
/// is not a valid UTI (such as `Pasteboard generated type`) appears by its raw
/// name only at pasteboard level and as a `dyn.` identifier per item.
/// This reads type names only, not item data.
pub fn read_types() -> Vec<String> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let mut out: Vec<String> = Vec::new();
    let mut push = |value: String| {
        if !out.contains(&value) {
            out.push(value);
        }
    };
    if let Some(types) = pasteboard.types() {
        for type_id in types.iter() {
            push(type_id.to_string());
        }
    }
    if let Some(items) = pasteboard.pasteboardItems() {
        for item in items.iter() {
            for type_id in item.types().iter() {
                push(type_id.to_string());
            }
        }
    }
    out
}

/// Plain text from the pasteboard. This is the first call that reads item data.
pub fn read_text() -> Option<String> {
    let pasteboard = NSPasteboard::generalPasteboard();
    unsafe { pasteboard.stringForType(NSPasteboardTypeString) }.map(|s| s.to_string())
}

/// Raw pasteboard access behaviour (macOS 15.4 and later).
pub fn access_behavior() -> Option<i64> {
    let pasteboard = NSPasteboard::generalPasteboard();
    Some(pasteboard.accessBehavior().0 as i64)
}

/// The frontmost application right now. Best effort: the lookup can return
/// nothing, for example right after launch.
pub fn frontmost_app() -> Option<SourceApp> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    Some(SourceApp {
        bundle_id: app.bundleIdentifier().map(|s| s.to_string()),
        name: app.localizedName().map(|s| s.to_string()),
    })
}

/// Replace the clipboard with plain text. Returns the change count produced by
/// the write and the count read straight after it, so the caller can tell
/// whether another app wrote in between. Returns `None` if the write failed.
pub fn write_text(text: &str) -> Option<WriteReceipt> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let produced = pasteboard.clearContents() as i64;
    let ok =
        unsafe { pasteboard.setString_forType(&NSString::from_str(text), NSPasteboardTypeString) };
    if !ok {
        return None;
    }
    Some(WriteReceipt {
        produced,
        current: pasteboard.changeCount() as i64,
    })
}

fn to_rect(r: NSRect) -> ScreenRect {
    ScreenRect {
        x: r.origin.x,
        y: r.origin.y,
        width: r.size.width,
        height: r.size.height,
    }
}

fn contains(r: NSRect, p: NSPoint) -> bool {
    p.x >= r.origin.x
        && p.x < r.origin.x + r.size.width
        && p.y >= r.origin.y
        && p.y < r.origin.y + r.size.height
}

/// Visible area of the display under the pointer. Must run on the main thread.
pub fn active_screen() -> Option<ActiveScreen> {
    let mtm = MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);
    // The first screen is the main display: origin (0, 0), menu bar on top.
    let main = screens.iter().next()?;
    let main_height = main.frame().size.height;
    let mouse = NSEvent::mouseLocation();
    let screen = screens
        .iter()
        .find(|s| contains(s.frame(), mouse))
        .unwrap_or(main);
    Some(ActiveScreen {
        visible: to_rect(screen.visibleFrame()),
        main_height,
    })
}

/// PNG bytes of an installed app's icon, looked up by bundle identifier.
/// `None` when the app is not installed. Must run on the main thread.
pub fn app_icon_png(bundle_id: &str) -> Option<Vec<u8>> {
    let workspace = NSWorkspace::sharedWorkspace();
    let url = workspace.URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))?;
    let path = url.path()?;
    let image = workspace.iconForFile(&path);
    let tiff = image.TIFFRepresentation()?;
    let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
    let png = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(png.to_vec())
}
