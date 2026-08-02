//! Notice when an authored `.map` changes on disk.
//!
//! Polled `mtime`, not a filesystem-notification crate. The workflow is "save
//! in TrenchBroom, look at the viewport", so the only latency that matters is
//! human, and a quarter-second poll is well inside it. A watcher crate would
//! buy milliseconds nobody can perceive in exchange for a dependency, a
//! background thread, and the platform-specific duplicate-event handling that
//! comes with every one of them. Revisit if the corpus outgrows a stat sweep -
//! at 58 modules it is four times a second over a warm directory entry.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bevy::prelude::*;

/// How often the corpus is restatted.
pub const POLL_SECONDS: f32 = 0.25;

/// The watched directory and what it looked like last time.
#[derive(Resource)]
pub struct ModuleWatch {
    pub dir: PathBuf,
    pub stamps: BTreeMap<PathBuf, Option<SystemTime>>,
    pub countdown: f32,
    /// Bumped whenever the set of files or any mtime moves, so the rest of the
    /// app can react without repeating the comparison.
    pub generation: u64,
}

impl ModuleWatch {
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            stamps: BTreeMap::new(),
            countdown: 0.0,
            generation: 0,
        }
    }

    /// The `.map` files in the watched directory, sorted.
    ///
    /// A missing or unreadable directory yields nothing rather than panicking:
    /// the tool is often started before the path is right, and the panel says
    /// so more usefully than a crash.
    #[must_use]
    pub fn scan(&self) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("map"))
            .collect();
        paths.sort();
        paths
    }

    /// Restat and report whether anything moved.
    ///
    /// Returns true on a changed mtime, a new file, **or** a deleted one. The
    /// last matters: a module deleted while it is the one on screen has to stop
    /// being on screen, and only the comparison against the previous set can
    /// tell.
    pub fn poll(&mut self) -> bool {
        let mut current = BTreeMap::new();
        for path in self.scan() {
            let stamp = std::fs::metadata(&path)
                .and_then(|meta| meta.modified())
                .ok();
            current.insert(path, stamp);
        }
        if current == self.stamps {
            return false;
        }
        self.stamps = current;
        self.generation += 1;
        true
    }
}

/// Tick the poll timer and restat when it expires.
pub fn poll_watch(time: Res<Time>, mut watch: ResMut<ModuleWatch>) {
    watch.countdown -= time.delta_secs();
    if watch.countdown > 0.0 {
        return;
    }
    watch.countdown = POLL_SECONDS;
    watch.poll();
}

/// Where the authored corpus lives, relative to the repo root.
#[must_use]
pub fn default_dir() -> PathBuf {
    Path::new("assets/tiles/authored").to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tiles/authored")
    }

    /// The first poll must report a change, or a tool started against an
    /// existing corpus would show nothing until someone edited a file.
    #[test]
    fn the_first_poll_reports_the_corpus_it_found() {
        let mut watch = ModuleWatch::new(corpus());
        assert!(watch.poll(), "the first poll must report the initial scan");
        assert!(!watch.stamps.is_empty(), "no .map files found");
        assert_eq!(watch.generation, 1);
    }

    /// A quiet corpus must stay quiet. A watcher that reports a change every
    /// tick would re-parse 58 modules four times a second and hide real edits
    /// in the noise.
    #[test]
    fn an_unchanged_corpus_reports_no_change() {
        let mut watch = ModuleWatch::new(corpus());
        watch.poll();
        let generation = watch.generation;
        assert!(!watch.poll(), "an unchanged corpus must report no change");
        assert_eq!(watch.generation, generation);
    }

    /// A missing directory is a normal startup state, not a crash.
    #[test]
    fn a_missing_directory_is_empty_rather_than_fatal() {
        let watch = ModuleWatch::new(corpus().join("definitely-not-here"));
        assert!(watch.scan().is_empty());
    }

    /// Only `.map` files. The directory also holds TrenchBroom autosaves, and
    /// one of those broke `tilec audit` once already.
    #[test]
    fn only_map_files_are_watched() {
        let watch = ModuleWatch::new(corpus());
        for path in watch.scan() {
            assert_eq!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("map"),
                "{} is not a .map",
                path.display()
            );
        }
    }
}
