// aerospace-watchdog -- restarts AeroSpace when it runs without its control socket.
//
// AeroSpace serves its CLI on /tmp/bobko.aerospace-<user>.sock. On 2026-10-15 that
// file disappeared at ~02:38 while AeroSpace kept running; other sockets in /tmp
// were untouched and the deleter was never found. Hotkeys kept working, because
// they are handled in-process, but every `aerospace` command failed with "Can't
// connect to AeroSpace server", so sketchybar's workspace pills froze. AeroSpace
// binds the socket once at startup and never recreates it; only a restart does.
//
// The check is the socket file, not a CLI round trip: AeroSpace drops replies
// during display reconfiguration, and restarting it for that would be worse than
// the bug.

use kennel_guest_sdk::{
    host_log, host_notify, host_spawn, host_state_get, host_state_set, kennel_extension,
    Manifest, Status,
};

// One probe, one line out. `id` because guest code has no environment. `-U` keeps
// another logged-in user's AeroSpace from counting as ours: matching it would
// relaunch an AeroSpace this user had quit.
// pgrep exits 1 for no match and 2 or 3 for its own errors; only 1 means "not
// running". Any other failure leaves the probe's exit code nonzero, which
// `decide` reads as Unknown instead of as healthy.
const PROBE: &str = "pgrep -x -U \"$(id -u)\" AeroSpace >/dev/null; rc=$?; \
    if [ $rc -eq 1 ]; then echo not_running; exit 0; fi; [ $rc -eq 0 ] || exit $rc; \
    if [ -S \"/tmp/bobko.aerospace-$(id -un).sock\" ]; then echo ok; else echo no_socket; fi";

// Exits 0 once this user's AeroSpace is gone. After 5s of SIGTERM it sends
// SIGKILL and gives that 1s; still alive then exits 1. Fits the 8s host limit.
// As in PROBE, only pgrep status 1 means gone; its own errors exit the script.
const WAIT_GONE: &str = "gone() { pgrep -x -U \"$(id -u)\" AeroSpace >/dev/null; rc=$?; \
    [ $rc -eq 1 ] && return 0; [ $rc -eq 0 ] && return 1; exit $rc; }; \
    for _ in 1 2 3 4 5 6 7 8 9 10; do gone && exit 0; sleep 0.5; done; \
    pkill -9 -x -U \"$(id -u)\" AeroSpace; sleep 1; gone";

// AeroSpace binds the socket shortly after launch, so a just-started AeroSpace
// looks like "running, no socket" for a moment. Two consecutive ticks (~30s at a
// 15s interval) rule that out.
const CONFIRM_TICKS: u32 = 2;

const STATE_KEY: &str = "consecutive_missing";

fn my_manifest() -> Manifest {
    Manifest {
        name: "aerospace-watchdog",
        version: "0.1.0",
        description: "Restarts AeroSpace when it runs without its control socket",
        interval_secs: 15,
        capabilities: vec!["spawn", "state", "log", "notify"],
        privileged_commands: vec![],
        ..Default::default()
    }
}

#[derive(Debug, PartialEq)]
pub enum Decision {
    // Socket present, or AeroSpace not running (quitting it is the user's call,
    // not a fault). Reset the counter.
    Healthy,
    // Socket missing, not yet confirmed. Carries the new counter value.
    Confirming(u32),
    // Missing on `CONFIRM_TICKS` consecutive ticks -- restart AeroSpace.
    Restart(String),
    // The probe could not be read. Never a restart.
    Unknown(String),
}

pub fn decide(exit_code: i32, stdout: &str, consecutive: u32) -> Decision {
    if exit_code != 0 {
        return Decision::Unknown(format!("probe exited {exit_code}"));
    }
    match stdout.lines().next().map(str::trim) {
        Some("ok") | Some("not_running") => Decision::Healthy,
        Some("no_socket") => {
            let n = consecutive + 1;
            if n < CONFIRM_TICKS {
                Decision::Confirming(n)
            } else {
                Decision::Restart(format!(
                    "AeroSpace is running without its control socket ({n} consecutive checks)"
                ))
            }
        }
        _ => Decision::Unknown(format!("unexpected probe output {:?}", stdout.trim())),
    }
}

fn my_check() -> Status {
    let result = host_spawn("/bin/sh", &["-c", PROBE]);
    let consecutive: u32 = host_state_get(STATE_KEY).parse().unwrap_or(0);

    match decide(result.exit_code, &result.stdout, consecutive) {
        Decision::Healthy => {
            if consecutive > 0 {
                host_log("info", "AeroSpace socket is back");
            }
            host_state_set(STATE_KEY, "0");
            Status::Healthy
        }
        Decision::Confirming(n) => {
            host_state_set(STATE_KEY, &n.to_string());
            host_log("info", &format!("AeroSpace socket missing (check {n} of {CONFIRM_TICKS})"));
            Status::Healthy
        }
        Decision::Restart(detail) => {
            host_state_set(STATE_KEY, &(consecutive + 1).to_string());
            host_log("warn", &detail);
            Status::Unhealthy(detail)
        }
        Decision::Unknown(why) => {
            host_log("warn", &format!("cannot tell whether AeroSpace has its socket: {why}; {}", result.stderr.trim()));
            // An unreadable tick breaks the streak: two misses with an unknown
            // between them are not consecutive.
            host_state_set(STATE_KEY, "0");
            Status::Healthy
        }
    }
}

fn my_fix() {
    host_log("warn", "restarting AeroSpace");
    host_spawn("/usr/bin/killall", &["AeroSpace"]);
    // `open` while the old instance is still quitting only activates it, and then
    // it exits. Do not launch until it is confirmed gone.
    let wait = host_spawn("/bin/sh", &["-c", WAIT_GONE]);
    if wait.exit_code != 0 {
        // The streak is kept, so the next check after the cooldown tries again.
        host_log("error", &format!("old AeroSpace did not exit (wait exited {}); not relaunching", wait.exit_code));
        host_notify("AeroSpace did not restart", "The old instance would not exit. Will retry.");
        return;
    }
    let open = host_spawn("/usr/bin/open", &["-b", "bobko.aerospace"]);
    // Start the confirmation over, so the new AeroSpace gets a full debounce
    // window to bind its socket before another restart is considered.
    host_state_set(STATE_KEY, "0");
    if open.exit_code != 0 {
        // After this, the probe reads "not running", which is healthy by design,
        // so nothing retries. Say so instead of claiming a restart.
        let why = format!("open -b bobko.aerospace exited {}: {}", open.exit_code, open.stderr.trim());
        host_log("error", &format!("AeroSpace was stopped but did not start again: {why}"));
        host_notify("AeroSpace did not restart", "It was stopped to recover its control socket, but failed to launch. Start it by hand.");
        return;
    }
    host_notify("AeroSpace restarted", "Its control socket was gone, so the bar could not read workspaces.");
}

kennel_extension!(my_manifest, my_check, my_fix);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_present_socket_is_healthy() {
        assert_eq!(decide(0, "ok\n", 0), Decision::Healthy);
        assert_eq!(decide(0, "ok\n", 1), Decision::Healthy, "recovery resets the counter");
    }

    // The user quit AeroSpace. Starting it again is not this watchdog's job.
    #[test]
    fn aerospace_not_running_is_healthy() {
        assert_eq!(decide(0, "not_running\n", 3), Decision::Healthy);
    }

    // The reported failure: AeroSpace up, socket gone.
    #[test]
    fn a_missing_socket_is_confirmed_then_restarted() {
        assert_eq!(decide(0, "no_socket\n", 0), Decision::Confirming(1));
        match decide(0, "no_socket\n", 1) {
            Decision::Restart(d) => assert!(d.contains("without its control socket"), "{d}"),
            other => panic!("expected a restart on the second consecutive check, got {other:?}"),
        }
    }

    // A just-launched AeroSpace has not bound its socket yet.
    #[test]
    fn a_single_bad_tick_never_restarts_anything() {
        assert_eq!(decide(0, "no_socket", 0), Decision::Confirming(1));
    }

    #[test]
    fn an_unusable_probe_never_triggers_a_restart() {
        assert!(matches!(decide(-1, "", 5), Decision::Unknown(_)), "host timeout");
        assert!(matches!(decide(0, "", 5), Decision::Unknown(_)));
        assert!(matches!(decide(0, "garbage", 5), Decision::Unknown(_)));
        assert!(matches!(decide(1, "no_socket", 5), Decision::Unknown(_)),
                "a nonzero exit wins over whatever is on stdout");
    }
}
