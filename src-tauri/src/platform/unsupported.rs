//! Placeholder for platforms other than macOS. Clipshelf is macOS only for now.
//! These stubs keep the crate and its unit tests building elsewhere.

use super::{ActiveScreen, SourceApp, WriteReceipt};

pub fn change_count() -> i64 {
    0
}
pub fn read_types() -> Vec<String> {
    Vec::new()
}
pub fn read_text() -> Option<String> {
    None
}
pub fn access_behavior() -> Option<i64> {
    None
}
pub fn frontmost_app() -> Option<SourceApp> {
    None
}
pub fn write_text(_text: &str) -> Option<WriteReceipt> {
    None
}
pub fn active_screen() -> Option<ActiveScreen> {
    None
}
pub fn app_icon_png(_bundle_id: &str) -> Option<Vec<u8>> {
    None
}
