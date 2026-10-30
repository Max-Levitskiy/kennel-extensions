// The question this extension exists to answer: "is there actually a sketchybar
// on every screen that should have one?"
//
// Every cheap way of asking is a lie. During the outage this was written for,
// sketchybar was running, answering `--query bar`, and reporting `drawing=on` /
// `hidden=off` -- while the bar was not on screen. The only signal that tracks
// what a human sees is the geometry the window server hands out.

use kennel_guest_sdk::{Display, Space, Window};

// A healthy bar measures 0.99-1.00 of the display's width (it insets by the
// config's `margin`); the failures seen so far leave nothing or 1x1 stubs.
pub const MIN_COVERAGE: f64 = 0.5;

// sketchybar parks the item windows it is not drawing at (-9999, -9999), and the
// window server reports them as on screen.
const PARKED_COORD: f64 = -9000.0;

fn is_bar_window(w: &Window) -> bool {
    w.onscreen && w.bounds.w > 1.0 && w.bounds.h > 1.0 && w.bounds.x > PARKED_COORD && w.bounds.y > PARKED_COORD
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayVerdict {
    pub id: u32,
    pub builtin: bool,
    pub width: f64,
    pub height: f64,
    pub coverage: f64,
    pub has_bar: bool,
    // macOS draws no sketchybar on a native fullscreen space.
    pub fullscreen: bool,
}

// Widest bar window on the display, as a fraction of its width. The widest one
// is the bar's background; item windows are narrow slices on top of it.
fn coverage(display: &Display, windows: &[Window]) -> f64 {
    if display.bounds.w <= 0.0 {
        return 0.0;
    }
    windows
        .iter()
        .filter(|w| is_bar_window(w) && w.bounds.intersects(&display.bounds))
        .map(|w| w.bounds.w / display.bounds.w)
        .fold(0.0, f64::max)
}

// `windows` must already be sketchybar's own -- the host filters by owner.
pub fn verdict(displays: &[Display], windows: &[Window]) -> Vec<DisplayVerdict> {
    displays
        .iter()
        .map(|d| {
            let c = coverage(d, windows);
            DisplayVerdict {
                id: d.id,
                builtin: d.builtin,
                width: d.bounds.w,
                height: d.bounds.h,
                coverage: c,
                has_bar: c >= MIN_COVERAGE,
                fullscreen: d.space == Space::Fullscreen,
            }
        })
        .collect()
}

// Displays that should show a bar, and how many of those do.
pub fn summary(v: &[DisplayVerdict]) -> (usize, usize) {
    let expected: Vec<_> = v.iter().filter(|d| !d.fullscreen).collect();
    let with_bar = expected.iter().filter(|d| d.has_bar).count();
    (expected.len(), with_bar)
}

pub fn describe(d: &DisplayVerdict) -> String {
    let bar = if d.fullscreen {
        "fullscreen"
    } else if d.has_bar {
        "present"
    } else {
        "MISSING"
    };
    format!(
        "display id={} builtin={} {:.0}x{:.0} coverage={:.0}% bar={bar}",
        d.id,
        d.builtin as u8,
        d.width,
        d.height,
        d.coverage * 100.0
    )
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use kennel_guest_sdk::Rect;

    pub fn display(id: u32, x: f64, y: f64, w: f64, h: f64) -> Display {
        Display { id, uuid: format!("uuid-{id}"), bounds: Rect { x, y, w, h }, builtin: id == 1, space: Space::Desktop }
    }

    pub fn window(x: f64, y: f64, w: f64, h: f64) -> Window {
        Window { owner: "sketchybar".into(), bounds: Rect { x, y, w, h }, onscreen: true, layer: -20 }
    }

    // A real bar insets by the config's `margin`, so it is a few px narrower than the display.
    pub fn bar_on(d: &Display) -> Window {
        window(d.bounds.x + 4.0, d.bounds.y + 4.0, d.bounds.w - 8.0, 39.0)
    }

    pub fn two_displays() -> Vec<Display> {
        vec![display(1, 0.0, 0.0, 1512.0, 982.0), display(2, -73.0, -1050.0, 1680.0, 1050.0)]
    }

    #[test]
    fn a_full_width_bar_on_every_display_is_healthy() {
        let ds = two_displays();
        let v = verdict(&ds, &[bar_on(&ds[0]), bar_on(&ds[1])]);
        assert!(v.iter().all(|d| d.has_bar), "{v:?}");
        assert!(v[0].coverage > 0.98);
    }

    #[test]
    fn a_bar_missing_from_one_display_is_caught_even_though_the_other_is_fine() {
        let ds = two_displays();
        let v = verdict(&ds, &[bar_on(&ds[1])]);
        assert!(!v[0].has_bar);
        assert!(v[1].has_bar);
    }

    #[test]
    fn parked_offscreen_item_windows_never_count_as_a_bar() {
        let ds = vec![display(1, 0.0, 0.0, 1512.0, 982.0)];
        let ws: Vec<Window> = (0..30).map(|_| window(-9999.0, -9999.0, 1512.0, 39.0)).collect();
        assert!(!verdict(&ds, &ws)[0].has_bar);
    }

    #[test]
    fn collapsed_zero_size_windows_never_count_as_a_bar() {
        let ds = vec![display(1, 0.0, 0.0, 1512.0, 982.0)];
        assert!(!verdict(&ds, &[window(0.0, 0.0, 0.0, 0.0)])[0].has_bar);
    }

    #[test]
    fn an_offscreen_bar_window_does_not_count() {
        let ds = vec![display(1, 0.0, 0.0, 1512.0, 982.0)];
        let mut w = bar_on(&ds[0]);
        w.onscreen = false;
        assert!(!verdict(&ds, &[w])[0].has_bar);
    }

    #[test]
    fn a_bar_on_a_different_display_does_not_satisfy_this_one() {
        let ds = two_displays();
        assert_eq!(verdict(&ds, &[bar_on(&ds[1])])[0].coverage, 0.0);
    }

    #[test]
    fn a_narrow_leftover_sliver_is_not_a_bar() {
        let ds = vec![display(1, 0.0, 0.0, 1512.0, 982.0)];
        assert!(!verdict(&ds, &[window(0.0, 4.0, 200.0, 39.0)])[0].has_bar);
    }

    #[test]
    fn a_fullscreen_display_without_a_bar_is_not_counted_as_missing() {
        let mut ds = two_displays();
        ds[0].space = Space::Fullscreen;
        assert_eq!(summary(&verdict(&ds, &[bar_on(&ds[1])])), (1, 1));
    }

    #[test]
    fn a_fullscreen_display_does_not_hide_a_missing_bar_elsewhere() {
        let mut ds = two_displays();
        ds[0].space = Space::Fullscreen;
        assert_eq!(summary(&verdict(&ds, &[])), (1, 0));
    }

    #[test]
    fn an_unknown_space_still_expects_a_bar() {
        let mut ds = vec![display(1, 0.0, 0.0, 1512.0, 982.0)];
        ds[0].space = Space::Unknown;
        assert_eq!(summary(&verdict(&ds, &[])), (1, 0));
    }
}
