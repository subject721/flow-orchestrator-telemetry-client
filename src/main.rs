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

    let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_io()
                .enable_time()
                .worker_threads(1)
                .build()
                .unwrap(); /*Runtime::new().unwrap()*/

    let mut metric_backend = backend::Backend::new();

    let connect_result = runtime.block_on(async {
        metric_backend.connect(args.endpoint_addr).await
    });

    if connect_result.is_err() {
        println!("Connection failed: {:?}", connect_result.err());

        return Err(Box::new(backend::Error{msg: "Nix connection".to_string()}))
    }

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

    runtime.shutdown_timeout(Duration::from_secs(5));

    Ok(())
}
