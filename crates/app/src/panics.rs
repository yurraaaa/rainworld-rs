//! Panics treated as Unity treats exceptions: logged with their backtrace, and the
//! window's frame catches them so the rest of that update or draw is skipped and the
//! game keeps running. A location that panics again is logged with a count (at 1, 10,
//! 100, ...) instead of a fresh backtrace.
//!
//! Mirrors: - (port-only: Unity's exception logging)

use std::backtrace::Backtrace;
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe, PanicHookInfo};
use std::sync::LazyLock;

use parking_lot::Mutex;

/// Times each panic location has panicked.
static SEEN: LazyLock<Mutex<HashMap<String, u64>>> = LazyLock::new(Mutex::default);

pub struct Panics;

impl Panics {
    /// Replaces the default hook (stderr, backtrace only with `RUST_BACKTRACE`) with one
    /// that logs every panic, with a backtrace the first time at each location.
    pub fn install_hook() {
        panic::set_hook(Box::new(Self::hook));
    }

    fn hook(info: &PanicHookInfo<'_>) {
        let location = info
            .location()
            .map_or_else(|| "<unknown>".to_owned(), ToString::to_string);
        let message = info
            .payload_as_str()
            .unwrap_or("<non-string panic payload>");
        let thread = std::thread::current();
        let thread = thread.name().unwrap_or("<unnamed>");
        let count = {
            let mut seen = SEEN.lock();
            let n = seen.entry(location.clone()).or_default();
            *n += 1;
            *n
        };
        if count == 1 {
            log::error!(
                "panic in thread '{thread}' at {location}: {message}\n{}",
                Backtrace::force_capture()
            );
        } else if 10u64.pow(count.ilog10()) == count {
            log::error!("panic at {location} again ({count} times): {message}");
        }
    }

    /// Runs `f`; a panic in it is logged by the hook and `None` returned, as an exception
    /// ends a Unity `Update` without ending the game.
    pub fn guard<R>(f: impl FnOnce() -> R) -> Option<R> {
        panic::catch_unwind(AssertUnwindSafe(f)).ok()
    }
}


#[cfg(test)]
mod tests;
