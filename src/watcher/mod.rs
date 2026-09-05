use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

/// Starts a portable notify watcher and debounces bursts produced by editors.
/// The callback receives only paths; analysis policy remains owned by the API.
pub fn watch_project(
    path: &Path,
    mut on_change: impl FnMut(Vec<String>) + Send + 'static,
) -> Result<RecommendedWatcher> {
    let (sender, receiver) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    })?;
    watcher.watch(path, RecursiveMode::Recursive)?;
    std::thread::spawn(move || {
        let mut pending = Vec::new();
        let mut last = Instant::now();
        loop {
            match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(Ok(event)) => {
                    for path in event.paths {
                        let text = path.to_string_lossy();
                        if !ignored(&text) {
                            pending.push(text.to_string());
                        }
                    }
                    last = Instant::now();
                }
                Ok(Err(_)) => {}
                Err(mpsc::RecvTimeoutError::Timeout)
                    if !pending.is_empty() && last.elapsed() >= Duration::from_millis(500) =>
                {
                    pending.sort();
                    pending.dedup();
                    on_change(std::mem::take(&mut pending));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    });
    Ok(watcher)
}
fn ignored(path: &str) -> bool {
    path.split(['/', '\\']).any(|part| {
        matches!(
            part,
            ".git"
                | "target"
                | "node_modules"
                | "dist"
                | "build"
                | ".next"
                | "vendor"
                | "coverage"
                | "cache"
                | ".code-atlas"
                | "graphify-out"
        )
    }) || path.ends_with(".sqlite")
        || path.ends_with(".sqlite-wal")
        || path.ends_with(".sqlite-shm")
}
