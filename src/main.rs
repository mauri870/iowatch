use std::convert::TryInto;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = iowatch::Cli::parse();
    let iowatch: iowatch::IoWatch = cli.try_into()?;
    iowatch.run()
}
