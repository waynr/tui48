use std::error::Error;
use std::io::stdout;

use anyhow::Result;
use rand::thread_rng;

mod engine;
mod error;
mod tui;
mod tui48;

use engine::board::Board;
use tui::crossterm::{Crossterm, CrosstermEvents};
use tui48::{init, Tui48};

fn main() -> Result<()> {
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
        .level(log::LevelFilter::Trace)
        .chain(fern::log_file("./output.log")?)
        .apply()?;

    log::info!("log test");

    init()?;

    tui48.run()?;
    // match tui48.run() {
    //     Err(e) => {
    //         eprintln!("{}", e);
    //     },
    //     Ok(_) => eprintln!("everything okay!"),
    // }

    Ok(())
}
