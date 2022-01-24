#![allow(dead_code)]

use std::error::Error;

use std::time::{Duration};

use clap::{ArgEnum, Parser};

use crate::aggregator::aggregator::MetricAggregator;
use crate::frontend::MetricFrontend;
use crate::gui_frontend::GraphicalFrontend;
use crate::terminal_frontend::{TerminalFrontend};

mod aggregator;

mod backend;
mod common;
mod source;
mod frontend;

#[cfg(feature = "terminal_frontend")]
mod terminal_frontend;

#[cfg(feature = "graphical_frontend")]
mod gui_frontend;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum FrontEndOption {
    TEST,

    #[cfg(feature = "terminal_frontend")]
    TUI,

    #[cfg(feature = "graphical_frontend")]
    GUI
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {

    #[clap(short, long)]
    pub endpoint_addr : String,

    #[clap(arg_enum, short, long, default_value_t = FrontEndOption::TUI)]
    pub frontend : FrontEndOption
}


fn main() -> Result<(), Box<dyn Error>> {

    let args = Cli::parse();

    let mut metric_backend = backend::Backend::new();

    metric_backend.connect(args.endpoint_addr).unwrap();

    if args.frontend == FrontEndOption::TEST {
        loop {
            println!("last timestamp: {}", metric_backend.get_last_timestamp());

            metric_backend.visit_metrics(|m| {
                println!("{}", m);
            });

            std::thread::sleep(Duration::from_millis(250));
        }
    } else if args.frontend == FrontEndOption::TUI {
        let frontend = TerminalFrontend::create(metric_backend)?;

        frontend.run()?;
    } else if args.frontend == FrontEndOption::GUI {
        let frontend = GraphicalFrontend::create(metric_backend)?;

        frontend.run()?;
    }

    Ok(())
}
