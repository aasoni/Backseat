use std::path::Path;
use std::time::Duration;

use notify_debouncer_full::notify::RecommendedWatcher;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

use crate::error::{Error, Result};

/// Keeps a debounced watch alive on a round directory; dropping it stops the
/// watch. Fires `on_change` after quiet periods of file activity.
pub struct RoundWatcher {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
}

pub fn watch_round(dir: &Path, on_change: impl Fn() + Send + 'static) -> Result<RoundWatcher> {
    let mut debouncer = new_debouncer(
        Duration::from_millis(250),
        None,
        move |res: DebounceEventResult| {
            if res.is_ok() {
                on_change();
            }
        },
    )
    .map_err(|e| Error::Other(format!("failed to start file watcher: {e}")))?;
    debouncer
        .watch(dir, notify_debouncer_full::notify::RecursiveMode::Recursive)
        .map_err(|e| Error::Other(format!("failed to watch {}: {e}", dir.display())))?;
    Ok(RoundWatcher {
        _debouncer: debouncer,
    })
}
