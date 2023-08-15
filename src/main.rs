use std::io::stdout;

use anyhow::Result;
use clap::Parser;
use rand::thread_rng;

mod engine;
mod error;
mod tui;
mod tui48;

use engine::board::Board;
use tui::crossterm::{Crossterm, CrosstermEvents};
use tui48::{init, Tui48};

#[derive(Debug, Parser)]
struct Cli {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let rng = thread_rng();
    let board = Board::new(rng);
    let w = stdout().lock();
    let renderer = Crossterm::new(Box::new(w))?;
    let event_source = CrosstermEvents::default();
    let tui48 = Tui48::new(board, renderer, event_source)?;
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {}] {}",
                record.level(),
                record.target(),
                message,
            ))
        })
        .level(cli.verbose.log_level_filter())
        .chain(fern::log_file("./output.log")?)
        .apply()?;

    init()?;

    tui48.run()?;

    Ok(())
}
