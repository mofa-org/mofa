// mofa-plugin-loader/src/watcher.rs
//
// File-system watcher that detects `.so` / `.dylib` changes and sends
// HotReloadEvents on a channel.

use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};

/// Events the shell reacts to.
#[derive(Debug)]
pub enum HotReloadEvent {
    Updated(PathBuf),
    Removed(PathBuf),
}

/// Spawn a background thread watching `plugin_dir` for shared-library changes.
///
/// Returns a `Receiver` the shell polls / selects on.
///
/// The watcher is kept alive by the returned `WatcherGuard`; drop it to stop
/// watching.
pub struct WatcherGuard {
    _watcher: RecommendedWatcher,
}

pub fn watch_plugin_dir(
    plugin_dir: &Path,
) -> Result<(WatcherGuard, Receiver<HotReloadEvent>), notify::Error> {
    let (tx, rx): (Sender<HotReloadEvent>, Receiver<HotReloadEvent>) =
        mpsc::channel();

    let tx_clone = tx.clone();

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            match res {
                Ok(Event {
                    kind: EventKind::Modify(_) | EventKind::Create(_),
                    paths,
                    ..
                }) => {
                    for path in paths {
                        if is_shared_lib(&path) {
                            tracing::debug!(
                                path = %path.display(),
                                "hot-reload: detected library update"
                            );
                            tx_clone
                                .send(HotReloadEvent::Updated(path))
                                .ok();
                        }
                    }
                }
                Ok(Event {
                    kind: EventKind::Remove(_),
                    paths,
                    ..
                }) => {
                    for path in paths {
                        if is_shared_lib(&path) {
                            tracing::debug!(
                                path = %path.display(),
                                "hot-reload: detected library removal"
                            );
                            tx.send(HotReloadEvent::Removed(path)).ok();
                        }
                    }
                }
                Ok(_) => {} // other events ignored
                Err(e) => {
                    tracing::warn!(error = %e, "hot-reload watcher error");
                }
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )?;

    watcher.watch(plugin_dir, RecursiveMode::NonRecursive)?;

    tracing::info!(
        path = %plugin_dir.display(),
        "hot-reload watcher started"
    );

    Ok((WatcherGuard { _watcher: watcher }, rx))
}

/// Returns `true` for `.so` (Linux) and `.dylib` (macOS) files.
fn is_shared_lib(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some("so") | Some("dylib") => true,
        _ => false,
    }
}
