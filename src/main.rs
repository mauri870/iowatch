use anyhow::Result;
use std::sync::mpsc;
use std::time::Duration;

use iowatch::IoWatch;
use notify::{RecommendedWatcher, Watcher};
use structopt::StructOpt;

fn main() -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_millis(25)).unwrap();

    IoWatch::from_args().run(&rx, watcher)?;
    Ok(())
}
