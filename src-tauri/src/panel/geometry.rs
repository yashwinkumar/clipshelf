//! Where the panel goes. Pure arithmetic so it can be tested on any platform.
//!
//! AppKit places the origin at the bottom left of the main display. Tauri
//! logical coordinates put the origin at the top left, so the y value is
//! converted using the main display height.

use crate::platform::ScreenRect;

/// Preferred panel height in points, matching the reference layout.
pub const PANEL_HEIGHT: f64 = 332.0;
/// Gap between the panel and the edges of the visible area, in points.
pub const PANEL_MARGIN: f64 = 8.0;
/// Smallest panel drawn, so a short display still shows something usable.
const MIN_HEIGHT: f64 = 120.0;
const MIN_WIDTH: f64 = 320.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Full width strip along the bottom of `visible`, inset by `PANEL_MARGIN`.
pub fn panel_frame(visible: ScreenRect, main_height: f64) -> PanelFrame {
    panel_frame_with(visible, main_height, PANEL_MARGIN, PANEL_HEIGHT)
}

/// The same rule with the margin and preferred height given. The height shrinks
/// on short displays and never drops below `MIN_HEIGHT`.
pub fn panel_frame_with(
    visible: ScreenRect,
    main_height: f64,
    margin: f64,
    preferred_height: f64,
) -> PanelFrame {
    let width = (visible.width - 2.0 * margin).max(MIN_WIDTH);
    let height = preferred_height
        .min(visible.height - 2.0 * margin)
        .max(MIN_HEIGHT);
    let x = visible.x + margin;
    let bottom = visible.y + margin;
    let top = bottom + height;
    PanelFrame {
        x,
        y: main_height - top,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(x: f64, y: f64, w: f64, h: f64) -> ScreenRect {
        ScreenRect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn main_display_bottom_strip() {
        // 1512 x 982 display with the Dock and menu bar excluded from the visible area.
        let f = panel_frame(screen(0.0, 0.0, 1512.0, 902.0), 982.0);
        assert_eq!(f.x, PANEL_MARGIN);
        assert_eq!(f.width, 1512.0 - 2.0 * PANEL_MARGIN);
        assert_eq!(f.height, PANEL_HEIGHT);
        // Bottom edge sits PANEL_MARGIN above the visible bottom (AppKit y = 0).
        assert_eq!(f.y, 982.0 - (PANEL_MARGIN + PANEL_HEIGHT));
    }

    #[test]
    fn bottom_dock_moves_panel_up() {
        let f = panel_frame(screen(0.0, 70.0, 1512.0, 900.0), 982.0);
        assert_eq!(f.y, 982.0 - (70.0 + PANEL_MARGIN + PANEL_HEIGHT));
    }

    #[test]
    fn secondary_display_with_negative_origin() {
        let f = panel_frame(screen(-1920.0, 0.0, 1920.0, 1055.0), 982.0);
        assert_eq!(f.x, -1920.0 + PANEL_MARGIN);
    }

    #[test]
    fn short_display_shrinks_height() {
        let f = panel_frame_with(screen(0.0, 0.0, 800.0, 200.0), 200.0, 8.0, 332.0);
        assert_eq!(f.height, 184.0);
    }

    #[test]
    fn tiny_display_keeps_minimum_height() {
        let f = panel_frame_with(screen(0.0, 0.0, 800.0, 100.0), 100.0, 8.0, 332.0);
        assert_eq!(f.height, MIN_HEIGHT);
    }
}
