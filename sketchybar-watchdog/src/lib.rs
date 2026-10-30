// sketchybar-watchdog -- restarts sketchybar when the bar is not actually on screen.
//
// This used to infer health from a wall-clock gap ("a gap appeared, so the Mac
// slept, so kick sketchybar"). That model was wrong in both directions. It fired
// after every wake whether or not anything was broken, and -- the reason it was
// rewritten -- it was blind to the failure that actually loses the bar: a display
// being connected or disconnected. During one such outage sketchybar was running,
// answering `--query bar`, and reporting drawing=on/hidden=off, with the bar
// nowhere on screen. Process liveness, socket liveness and the bar's own flags
// were all green for 41 hours.
//
// So health is now measured the only way that matches what a human sees: ask the
// window server for sketchybar's actual window geometry, per display. That covers
// the wake case too, without having to guess at causes.

mod verdict;

use kennel_guest_sdk::{
    host_launchctl, host_log, host_spawn, host_state_get, host_state_set, host_window_server, kennel_extension,
    Manifest, Status, WindowServer,
};
use verdict::{describe, summary, verdict};

// A display change legitimately leaves the bar absent for a moment while
// sketchybar rebuilds one window per item. Requiring two consecutive bad ticks
// (~10s at a 5s interval) keeps the watchdog from kickstarting sketchybar every
// time a monitor is plugged in, which would be worse than the bug.
const CONFIRM_TICKS: u32 = 2;

const STATE_KEY: &str = "consecutive_missing";

fn my_manifest() -> Manifest {
    Manifest {
        name: "sketchybar-watchdog",
        version: "0.3.0",
        description: "Restarts sketchybar when the bar is not actually on screen",
        interval_secs: 5,
        capabilities: vec!["launchctl", "spawn", "state", "log", "window_server"],
        ..Default::default()
    }
}

#[derive(Debug, PartialEq)]
pub enum Decision {
    // Nothing to do; reset the confirmation counter.
    Healthy,
    // The bar is missing but not yet confirmed. Carries the new counter value.
    Confirming(u32),
    // Missing on `CONFIRM_TICKS` consecutive ticks -- restart, with this detail.
    Restart(String),
    // The window server could not be read. Deliberately NOT a restart: acting on
    // no information is how a watchdog turns into a restart loop.
    Unknown(String),
}

pub fn decide(snapshot: &Result<WindowServer, String>, consecutive: u32) -> Decision {
    let snapshot = match snapshot {
        Ok(s) => s,
        Err(e) => return Decision::Unknown(e.clone()),
    };
    // No display expects a bar: the lid is shut, every screen is asleep, or every
    // screen is showing a fullscreen app. Restarting sketchybar would just churn.
    let (expected, with_bar) = summary(&verdict(&snapshot.displays, &snapshot.windows));
    if with_bar >= expected {
        return Decision::Healthy;
    }
    let n = consecutive + 1;
    if n < CONFIRM_TICKS {
        Decision::Confirming(n)
    } else {
        Decision::Restart(format!("bar missing on {} of {} display(s) for {} consecutive checks", expected - with_bar, expected, n))
    }
}

fn details(snapshot: &Result<WindowServer, String>) -> String {
    match snapshot {
        Ok(s) => verdict(&s.displays, &s.windows).iter().map(describe).collect::<Vec<_>>().join("\n"),
        Err(_) => String::new(),
    }
}

fn my_check() -> Status {
    let snapshot = host_window_server("sketchybar");
    let consecutive: u32 = host_state_get(STATE_KEY).parse().unwrap_or(0);

    match decide(&snapshot, consecutive) {
        Decision::Healthy => {
            if consecutive > 0 {
                host_log("info", "bar is back on every display");
            }
            host_state_set(STATE_KEY, "0");
            Status::Healthy
        }
        Decision::Confirming(n) => {
            host_state_set(STATE_KEY, &n.to_string());
            // Still reported Healthy: this is the debounce window, and a display
            // change that resolves on its own must not show up as a fault.
            host_log("info", &format!("bar missing (check {n} of {CONFIRM_TICKS})\n{}", details(&snapshot)));
            Status::Healthy
        }
        Decision::Restart(detail) => {
            host_state_set(STATE_KEY, &(consecutive + 1).to_string());
            host_log("warn", &format!("{detail}\n{}", details(&snapshot)));
            Status::Unhealthy(detail)
        }
        Decision::Unknown(why) => {
            host_log("warn", &format!("cannot tell whether the bar is up: {why}"));
            Status::Healthy
        }
    }
}

fn my_fix() {
    let uid = host_spawn("/usr/bin/id", &["-u"]).stdout.trim().to_string();
    host_log("warn", "restarting sketchybar");
    host_launchctl("kickstart", &["-k", &format!("gui/{uid}/homebrew.mxcl.sketchybar")]);
    // Start the confirmation over, so the restart gets a full debounce window to
    // take effect before another one is considered.
    host_state_set(STATE_KEY, "0");
}

kennel_extension!(my_manifest, my_check, my_fix);

#[cfg(test)]
mod tests {
    use super::*;
    use kennel_guest_sdk::Space;
    use verdict::tests::{bar_on, display, two_displays};

    fn snapshot(displays: Vec<kennel_guest_sdk::Display>, bars_on: &[usize]) -> Result<WindowServer, String> {
        let windows = bars_on.iter().map(|&i| bar_on(&displays[i])).collect();
        Ok(WindowServer { displays, windows })
    }

    #[test]
    fn a_bar_on_every_display_is_healthy() {
        assert_eq!(decide(&snapshot(two_displays(), &[0, 1]), 0), Decision::Healthy);
        assert_eq!(decide(&snapshot(two_displays(), &[0, 1]), 5), Decision::Healthy, "recovery resets, whatever the counter was");
    }

    #[test]
    fn a_bar_missing_on_one_of_two_displays_is_confirmed_then_restarted() {
        assert_eq!(decide(&snapshot(two_displays(), &[1]), 0), Decision::Confirming(1));
        match decide(&snapshot(two_displays(), &[1]), 1) {
            Decision::Restart(d) => assert!(d.contains("1 of 2"), "{d}"),
            other => panic!("expected a restart on the second consecutive check, got {other:?}"),
        }
    }

    // Plugging in a monitor blanks the bar for a moment while sketchybar rebuilds
    // its windows. One bad tick must never be enough.
    #[test]
    fn a_single_bad_tick_never_restarts_anything() {
        assert_eq!(decide(&snapshot(two_displays(), &[]), 0), Decision::Confirming(1));
    }

    #[test]
    fn an_unreadable_window_server_never_triggers_a_restart() {
        assert!(matches!(decide(&Err("capability not granted".into()), 9), Decision::Unknown(_)));
    }

    #[test]
    fn no_active_displays_is_healthy_not_a_missing_bar() {
        assert_eq!(decide(&snapshot(vec![], &[]), 3), Decision::Healthy);
    }

    #[test]
    fn a_fullscreen_display_without_a_bar_never_restarts_sketchybar() {
        let mut ds = two_displays();
        ds[0].space = Space::Fullscreen;
        assert_eq!(decide(&snapshot(ds, &[1]), 5), Decision::Healthy);
    }

    #[test]
    fn every_display_fullscreen_is_healthy() {
        let mut ds = vec![display(1, 0.0, 0.0, 1512.0, 982.0)];
        ds[0].space = Space::Fullscreen;
        assert_eq!(decide(&snapshot(ds, &[]), 5), Decision::Healthy);
    }
}
