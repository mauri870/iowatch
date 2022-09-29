use anyhow::Result;
use clap::Parser;
use std::sync::mpsc;
use std::time::Duration;

use iowatch::IoWatch;
use notify::{RecommendedWatcher, Watcher};

fn main() -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_millis(25)).unwrap();

    IoWatch::parse().run(&rx, watcher)?;
    Ok(())
}
