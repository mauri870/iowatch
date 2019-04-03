use std::sync::mpsc;
use std::time::Duration;

use entr::Entr;
use structopt::StructOpt;
use notify::{RecommendedWatcher, Watcher};
use exitfailure::ExitFailure;

fn main() -> Result<(), ExitFailure> {
    let (tx, rx) = mpsc::channel();
    let watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(0)).unwrap();

    Entr::from_args().run(&rx, watcher)?;
    Ok(())
}
