use std::convert::TryInto;

use anyhow::Result;
use clap::Parser;
use log::debug;

fn main() -> Result<()> {
    env_logger::init();

    let cli = iowatch::Cli::parse();
    debug!("Parsed command line arguments: {:?}", cli);
    let iowatch: iowatch::IoWatch = cli.try_into()?;
    debug!("Starting iowatch with configuration: {:?}", iowatch);
    iowatch.run()
}
