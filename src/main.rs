use std::sync::mpsc;
use iowatch::IoWatch;
use structopt::StructOpt;
use notify::{RecommendedWatcher, Watcher};

fn main() -> Result<(), anyhow::Error> {
    let (tx, rx) = mpsc::channel();
    let w: RecommendedWatcher = RecommendedWatcher::new(tx).unwrap();

    IoWatch::from_args().run(&rx, w)?;
    Ok(())
}
