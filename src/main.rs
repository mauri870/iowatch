use anyhow::Result;
use iowatch::IoWatch;
use notify::{RecommendedWatcher, Watcher};
use std::sync::mpsc;
use structopt::StructOpt;

fn main() -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let w: RecommendedWatcher = RecommendedWatcher::new(tx).unwrap();

    IoWatch::from_args().run(&rx, w)?;
    Ok(())
}
