use anyhow::Result;
use clap::Parser;

use iowatch::IoWatch;

fn main() -> Result<()> {
    IoWatch::parse().run()?;
    Ok(())
}
