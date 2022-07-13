use std::collections::BTreeSet;
use egui_glow::{glow, EguiGlow};
use glutin::event::Event;
use glutin::event_loop::{ControlFlow, EventLoop};
use std::error::Error;
use std::fmt::{Display, Formatter};
use egui::{ScrollArea, Ui, WidgetText};
use egui::Key::P;
use egui::plot::{Corner, Legend, Line, Plot, Value, Values};
use glutin::platform::run_return::EventLoopExtRunReturn;
use glutin::platform::unix::x11::util::modifiers::Modifier;

use crate::MetricFrontend;

use crate::backend::Backend;
use crate::common::metric::Metric;

pub trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}

#[derive(Debug)]
pub struct FrontendError {
    msg: String,
}

pub struct GraphicalFrontendInternal {
    metric_backend: Backend,

    window: glutin::WindowedContext<glutin::PossiblyCurrent>,

    gl_context: glow::Context,

    egui: EguiGlow,

    selection: usize,

    metric_list: MetricWidget,
}

pub struct GraphicalFrontend {
    event_loop: EventLoop<()>,

    internal: GraphicalFrontendInternal,
}


struct MetricWidget {
    metrics: Vec<Metric>,

    selected_metric: BTreeSet<String>,
}

impl From<&dyn std::error::Error> for FrontendError {
    fn from(error: &dyn std::error::Error) -> Self {
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

impl std::error::Error for FrontendError {
    fn description(&self) -> &str {
        &self.msg
    }
}


impl GraphicalFrontend {
    pub fn create(metric_backend: Backend) -> Result<GraphicalFrontend, FrontendError> {
        let event_loop = glutin::event_loop::EventLoop::with_user_event();

        let (gl_window, context) = GraphicalFrontend::create_display(&event_loop);

        let egui = egui_glow::EguiGlow::new(&gl_window, &context);

        let event_proxy = event_loop.create_proxy();

        metric_backend.add_callback(move || {
            let res = event_proxy.send_event(());

            if res.is_err() {
                println!("Event loop closed");
            }
        });

        Ok(GraphicalFrontend {
            event_loop,
            internal: GraphicalFrontendInternal {
                metric_backend,
                window: gl_window,
                gl_context: context,
                egui,
                selection: 0,
                metric_list: MetricWidget::default(),
            },
        })
    }

    fn create_display(
        event_loop: &glutin::event_loop::EventLoop<()>,
    ) -> (
        glutin::WindowedContext<glutin::PossiblyCurrent>,
        glow::Context,
    ) {
        let window_builder = glutin::window::WindowBuilder::new()
            .with_resizable(true)
            .with_inner_size(glutin::dpi::LogicalSize {
                width: 800.0,
                height: 600.0,
            })
            .with_title("DamnUglyMetricClient");

        let gl_window = unsafe {
            glutin::ContextBuilder::new()
                .with_depth_buffer(0)
                .with_srgb(true)
                .with_stencil_buffer(0)
                .with_vsync(true)
                .build_windowed(window_builder, event_loop)
                .unwrap()
                .make_current()
                .unwrap()
        };

        let gl = unsafe { glow::Context::from_loader_function(|s| gl_window.get_proc_address(s)) };

        unsafe {
            use glow::HasContext as _;
            gl.enable(glow::FRAMEBUFFER_SRGB);
        }

        (gl_window, gl)
    }
}

impl Default for MetricWidget {
    fn default() -> Self {
        MetricWidget {
            metrics: vec!(),
            selected_metric: BTreeSet::new(),
        }
    }
}

impl View for MetricWidget {
    fn ui(&mut self, ui: &mut Ui) {
        let scroll_area = ScrollArea::vertical()
            .max_height(400.0)
            .auto_shrink([false, true]);

        scroll_area.show(ui, |ui| {
            ui.vertical(|ui| {
                let mut new_selection = None;
                let mut add_to_clipboard = false;

                {
                    //let selected_name = self.selected_metric.as_ref().map(|s| { s.as_str() }).unwrap_or("");

                    for metric in &self.metrics {
                        let response =
                            ui.selectable_label(self.selected_metric.contains(metric.get_label()), WidgetText::from(metric.to_string()).monospace());

                        if response.clicked() {
                            if ui.input().modifiers.shift {
                                if self.selected_metric.contains(metric.get_label()) {
                                    self.selected_metric.remove(metric.get_label());
                                } else {
                                    self.selected_metric.insert(metric.get_label().to_string());
                                }
                            } else if !self.selected_metric.contains(metric.get_label()) {
                                self.selected_metric.clear();
                                self.selected_metric.insert(metric.get_label().to_string());
                            }

                            new_selection = Some(metric.get_label());

                            if response.double_clicked() {
                                add_to_clipboard = true
                            }
                        }
                    }
                }

                if let Some(new_selection) = new_selection {
                    //self.selected_metric = Some(new_selection.to_string());

                    if add_to_clipboard {
                        let mut o = ui.output();

                        o.copied_text = new_selection.to_string();
                    }
                }
            });
        });

        ui.separator();
    }
}

impl MetricWidget {
    pub fn update_metrics(&mut self, metrics: Vec<Metric>) {
        self.metrics = metrics;
        self.metrics.sort_by(|a, b| {
            a.get_label().cmp(b.get_label())
        })
    }

    pub fn get_selection(&self) -> &BTreeSet<String> {
        &self.selected_metric
    }
}

impl GraphicalFrontendInternal {
    fn event_handle(&mut self, event: Event<'_, ()>, control_flow: &mut ControlFlow) {
        match event {
            // Platform-dependent event handlers to workaround a winit bug
            // See: https://github.com/rust-windowing/winit/issues/987
            // See: https://github.com/rust-windowing/winit/issues/1619
            glutin::event::Event::RedrawEventsCleared if cfg!(windows) => self.redraw(control_flow),
            glutin::event::Event::RedrawRequested(_) if !cfg!(windows) => self.redraw(control_flow),

            glutin::event::Event::WindowEvent { event, .. } => {
                use glutin::event::WindowEvent;
                if matches!(event, WindowEvent::CloseRequested | WindowEvent::Destroyed) {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                }

                if let glutin::event::WindowEvent::Resized(physical_size) = event {
                    self.window.resize(physical_size);
                }

                self.egui.on_event(&event);

                self.window.window().request_redraw(); // TODO: ask egui if the events warrants a repaint instead
            }
            glutin::event::Event::LoopDestroyed => {
                self.egui.destroy(&self.gl_context);
            }
            glutin::event::Event::UserEvent(()) => {
                self.redraw(control_flow);
            }

            _ => (),
        }
    }

    fn redraw(&mut self, control_flow: &mut ControlFlow) {
        let clear_color = [0.1, 0.1, 0.1];

        let mut quit = false;

        self.metric_list.update_metrics(self.metric_backend.map_metrics(|m| { m.clone() }));

        let needs_repaint = self.egui.run(self.window.window(), |egui_ctx| {
            egui::TopBottomPanel::top("top_panel").show(egui_ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            quit = true;
                        }
                    });
                });
            });

            egui::SidePanel::left("side_panel").show(egui_ctx, |ui| {
                //ui.heading("Flow Orchestrator Metric Client");
            });

            egui::CentralPanel::default().show(&egui_ctx, |ui| {
                ui.vertical_centered_justified(|ui| {
                    self.metric_list.ui(ui);

                    if !self.metric_list.get_selection().is_empty() {
                        //let mut history_data = vec!();
                        let max_history_len = 256;

                        let plot = Plot::new("metric_plot")
                                .legend(Legend::default().position(Corner::RightBottom))
                                .show_x(true)
                                .show_y(true).allow_drag(false).allow_zoom(false);

                        // for selected_metric_name in &self.metric_list.selected_metric {
                        //     if let Some(_limits) = self.metric_backend.get_metric_history(selected_metric_name, &mut history_data, max_history_len) {
                        //         let plot_data: Vec<_> = history_data.iter().map(|m| { Value::new(m.0, m.1) }).collect();
                        //
                        //         let lines = Line::new(Values::from_values(plot_data));
                        //
                        //         plot.show(ui, |plot_ui| {
                        //             plot_ui.line(lines.name(selection));
                        //         });
                        //     }
                        // }
                        let selected_metrics = &self.metric_list.selected_metric;

                        plot.show(ui, |plot_ui| {
                            let mut history_data = vec!();

                            for selected_metric_name in selected_metrics {
                                if let Some(_limits) = self.metric_backend.get_metric_history(selected_metric_name, &mut history_data, max_history_len) {
                                    let plot_data: Vec<_> = history_data.iter().map(|m| { Value::new(m.0, m.1) }).collect();

                                    let lines = Line::new(Values::from_values(plot_data));

                                    plot_ui.line(lines.name(selected_metric_name));
                                }
                            }
                        });
                    }
                });
            });
        });

        *control_flow = if quit {
            glutin::event_loop::ControlFlow::Exit
        } else if needs_repaint.0 {
            self.window.window().request_redraw();
            glutin::event_loop::ControlFlow::Poll
        } else {
            glutin::event_loop::ControlFlow::Wait
        };


        {
            unsafe {
                use glow::HasContext as _;
                self.gl_context.clear_color(clear_color[0], clear_color[1], clear_color[2], 1.0);
                self.gl_context.clear(glow::COLOR_BUFFER_BIT);
            }

            self.egui.paint(&self.window, &self.gl_context, needs_repaint.1);

            self.window.swap_buffers().unwrap();
        }
    }
}

impl MetricFrontend for GraphicalFrontend {
    fn run(mut self) -> Result<(), Box<dyn Error>> {
        let mut internal = self.internal;

        self.event_loop.run_return(move |event, _, control_flow| {
            internal.event_handle(event, control_flow);
        });


        Ok(())
    }
}
