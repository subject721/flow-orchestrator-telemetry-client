use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{event, execute};
use std::borrow::BorrowMut;
use std::collections::LinkedList;
use std::fmt::{Display, Formatter};
use std::io;
use std::io::{Error, Stdout};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::text::Span;
use tui::widgets::{Axis, Block, Borders, Cell, Chart, Dataset, GraphType, Row, Table, TableState};
use tui::{symbols, Frame, Terminal};

use crate::backend::{Backend, MetricAdapter};
use crate::common::metric::Metric;
use crate::frontend::MetricFrontend;

pub struct TerminalFrontendOptions {}

#[derive(Debug)]
pub struct FrontendError {
    msg: String,
}

struct MetricTableRowState {
    cells: [String; 3],
}

struct UiState {
    selection_id: usize,

    table_state: TableState,

    rows: Vec<MetricTableRowState>,

    current_metric_history_data: Vec<(f64, f64)>,

    current_metric_history_range: (f64, f64),

    current_metric_history_time_range: (f64, f64),

    graph_active: bool,
}

pub struct TerminalFrontend {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    backend: Backend,
}

impl From<std::io::Error> for FrontendError {
    fn from(error: Error) -> Self {
        FrontendError {
            msg: error.to_string(),
        }
    }
}

impl Display for FrontendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for FrontendError {}

impl MetricAdapter for MetricTableRowState {
    fn get_name(&self) -> &String {
        &self.cells[0]
    }

    fn update_current(&mut self, metric: &Metric) {
        self.cells[0] = metric.get_label().to_string();
        self.cells[1] = metric.get_value().to_string();
        self.cells[2] = metric.get_unit().to_string();
    }
}

// impl<'a> IntoIterator for MetricTableRowState {
//     type Item = &'a str;
//     type IntoIter = std::slice::Iter<'a, Self::Item>;
//
//     fn into_iter(self) -> Self::IntoIter {
//
//     }
// }

impl<'a> Into<Row<'a>> for &MetricTableRowState {
    fn into(self) -> Row<'a> {
        Row::new(self.cells.clone())
    }
}

impl UiState {
    pub fn new() -> UiState {
        UiState {
            selection_id: 0usize,
            table_state: TableState::default(),
            rows: Vec::new(),
            current_metric_history_data: Vec::new(),
            current_metric_history_range: (1.0f64, 1.0f64),
            current_metric_history_time_range: (0.0f64, 0.0f64),
            graph_active: false,
        }
    }

    pub fn select_next(&mut self) {
        if self.table_state.selected() == None {
            self.table_state.select(Some(0));
        } else {
            let current = self.table_state.selected().unwrap();

            if current < (self.rows.len() - 1) {
                self.table_state.select(Some(current + 1));
            }
        }
    }

    pub fn select_prev(&mut self) {
        if self.table_state.selected() == None {
            self.table_state.select(Some(0));
        } else {
            let current = self.table_state.selected().unwrap();

            if current > 0 {
                self.table_state.select(Some(current - 1));
            }
        }
    }

    pub fn select_none(&mut self) {
        self.table_state.select(None);
        self.graph_active = false;
    }

    pub fn update_from_backend(&mut self, metric_backend: &Backend) {
        self.rows = metric_backend.map_metrics(|metric| {
            let cells = [
                metric.get_label().to_string(),
                metric.get_value().to_string(),
                metric.get_unit().to_string(),
            ];

            MetricTableRowState { cells }
        });

        if let Some(selection) = self.table_state.selected() {
            if let Some(row_data) = self.rows.get(selection) {
                if let Some(limits) = metric_backend
                    .get_metric_history(&row_data.cells[0], &mut self.current_metric_history_data, 64)
                {
                    self.current_metric_history_range = limits;
                    self.current_metric_history_time_range.0 = self.current_metric_history_data[0].0;
                    self.current_metric_history_time_range.1 = self.current_metric_history_data[self.current_metric_history_data.len() - 1].0;
                    self.graph_active = true;
                }
            }
        }
    }
}

impl TerminalFrontend {
    fn on_tick(ui_state: &mut UiState, metric_backend: &Backend) {
        ui_state.update_from_backend(metric_backend);
    }

    fn ui<B: tui::backend::Backend>(f: &mut Frame<B>, ui_state: &mut UiState) {
        let size = f.size();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(size);

        let selected_style = Style::default().add_modifier(Modifier::REVERSED);
        let normal_style = Style::default().bg(Color::Blue);

        let header_cells = ["Name", "Value", "Unit"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Red)));

        let header = Row::new(header_cells)
            .style(normal_style)
            .height(1)
            .bottom_margin(1);

        let rows: Vec<Row> = ui_state.rows.iter().map(|e| e.into()).collect();

        let t = Table::new(rows)
            .header(header)
            .block(Block::default().borders(Borders::ALL).title("Metrics"))
            .highlight_style(selected_style)
            .widths(&[
                Constraint::Percentage(50),
                Constraint::Length(30),
                Constraint::Min(10),
            ]);

        f.render_stateful_widget(t, chunks[0], &mut ui_state.table_state);

        if ui_state.graph_active {
            let dataset = Dataset::default()
                .name("History")//.marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Red))
                .data(&ui_state.current_metric_history_data);

            let min_timestamp = ui_state.current_metric_history_time_range.0;
            let max_timestamp = ui_state.current_metric_history_time_range.1;

            let x_labels = vec![
                Span::styled(
                    format!("{:.2}", min_timestamp),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:.2}", max_timestamp),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ];

            // Create a 5% margin above and below actual value range
            let y_range = ui_state.current_metric_history_range.1 - ui_state.current_metric_history_range.0;
            let y_limit_min = ui_state.current_metric_history_range.0 - (y_range * 0.05);
            let y_limit_max = ui_state.current_metric_history_range.1 + (y_range * 0.05);

            let chart = Chart::new(vec![dataset])
                .block(
                    Block::default()
                        .title(Span::styled(
                            "History Data",
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL),
                )
                .x_axis(
                    Axis::default()
                        .title("X Axis")
                        .style(Style::default().fg(Color::Gray))
                        .labels(x_labels)
                        .bounds([min_timestamp, max_timestamp]),
                )
                .y_axis(
                    Axis::default()
                        .title("Y Axis")
                        .style(Style::default().fg(Color::Gray))
                        .labels(vec![
                            Span::styled(
                                format!("{:.1}", y_limit_min),
                                Style::default().add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                format!("{:.1}", y_limit_max),
                                Style::default().add_modifier(Modifier::BOLD),
                            ),
                        ])
                        .bounds([
                            y_limit_min,
                            y_limit_max,
                        ]),
                );

            f.render_widget(chart, chunks[1]);
        }
    }

    pub fn create(metric_backend: Backend) -> Result<TerminalFrontend, FrontendError> {
        let mut stdout = io::stdout();

        enable_raw_mode()?;

        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);

        let terminal = Terminal::new(backend).unwrap();

        Ok(TerminalFrontend {
            terminal,
            backend: metric_backend,
        })
    }
}

impl Drop for TerminalFrontend {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();

        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .unwrap();
    }
}

impl MetricFrontend for TerminalFrontend {
    fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
        let tick_rate = Duration::from_millis(250);

        let mut last_tick = Instant::now();

        let mut ui_state = UiState::new();

        loop {
            {
                let ui_state_l = ui_state.borrow_mut();

                self.terminal.draw(|f| {
                    TerminalFrontend::ui(f, ui_state_l);
                })?;
            }

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if crossterm::event::poll(timeout).unwrap_or(false) {
                if let Event::Key(key) = event::read().unwrap() {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Down => {
                            ui_state.select_next();
                        }
                        KeyCode::Up => {
                            ui_state.select_prev();
                        }
                        KeyCode::Left => {
                            ui_state.select_none();
                        }
                        _ => {}
                    }
                }
            }
            if last_tick.elapsed() >= tick_rate {
                TerminalFrontend::on_tick(&mut ui_state, &self.backend);

                last_tick = Instant::now();
            }
        }

        self.backend.disconnect();

        Ok(())
    }
}
