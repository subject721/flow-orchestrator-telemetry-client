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

mod aggregator;

mod backend;
mod common;
mod source;
mod frontend;

#[cfg(feature = "terminal_frontend")]
mod terminal_frontend;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum FrontEndOption {
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

fn ui<B: Backend>(f: &mut Frame<B>, backend: &backend::Backend) {

    let size = f.size();

    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let normal_style = Style::default().bg(Color::Blue);

    let header_cells = ["Name", "Value", "Unit"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Red)));

    let header = Row::new(header_cells)
        .style(normal_style)
        .height(1)
        .bottom_margin(1);

    let rows = backend.map_metrics(|metric|{
        let cells = [metric.get_label().to_string(), metric.get_value().to_string(), metric.get_unit().to_string()];

        Row::new(cells)
    });

    let t = Table::new(rows)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Metrics"))
        .highlight_style(selected_style).widths(&[
            Constraint::Percentage(50),
            Constraint::Length(30),
            Constraint::Min(10),
        ]);

    f.render_widget(t, size);

    // let metric_txt = backend.map_metrics(|metric| Spans::from(metric.to_string()));
    //
    // let paragraph =
    //     Paragraph::new(metric_txt).block(Block::default().title("Metrics").borders(Borders::ALL));
    //
    // f.render_widget(paragraph, size);
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut metric_backend: backend::Backend,
    tick_rate: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();

    loop {

        terminal.draw(|f| ui(f, &metric_backend))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(tick_rate).unwrap_or(false) {
            if let Event::Key(key) = event::read().unwrap() {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            // Update stuff that needs to be updated in ui loop
            last_tick = Instant::now();
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {


    let args = Cli::parse();

    let mut metric_backend = backend::Backend::new();

    metric_backend.connect(args.endpoint_addr).unwrap();

    enable_raw_mode()?;

    let mut stdout = io::stdout();

    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend).unwrap();

    let tick_rate = Duration::from_millis(100);

    let result = run_app(&mut terminal, metric_backend, tick_rate);

    disable_raw_mode()?;

    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    terminal.show_cursor()?;

    Ok(())
}
