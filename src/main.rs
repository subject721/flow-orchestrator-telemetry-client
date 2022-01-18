#![allow(dead_code, unused_imports)]

use clap;
use clap::{Parser, ArgEnum};

use crate::aggregator::aggregator::MetricAggregator;
use crate::common::message::MetricMessage;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{event, execute};
use std::error::Error;
use std::fmt::Debug;
use std::io;
use std::io::ErrorKind;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::{select, task};
use tui::backend::{Backend, CrosstermBackend};
use tui::text::Spans;
use tui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use tui::{Frame, Terminal};
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use zeromq::{Socket, SocketRecv, ZmqResult};
use crate::frontend::MetricFrontend;
use crate::terminal_frontend::{TerminalFrontend, TerminalFrontendOptions};

mod aggregator;

mod backend;
mod common;
mod source;
mod frontend;

#[cfg(feature = "terminal_frontend")]
mod terminal_frontend;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum FrontEndOption {
    TRIVIAL,

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

    if args.frontend == FrontEndOption::TRIVIAL {
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
    }


    Ok(())
}
