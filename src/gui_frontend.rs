use egui_glow::{glow, EguiGlow};
use glutin::event::Event;
use glutin::event_loop::{ControlFlow, EventLoop};
use glutin::{Context, ContextCurrentState, WindowedContext};
use std::any::TypeId;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::ops::Add;
use std::time::{Duration, Instant};
use egui::Align;
use glutin::platform::run_return::EventLoopExtRunReturn;

use crate::MetricFrontend;

use crate::backend::Backend;

#[derive(Debug)]
pub struct FrontendError {
    msg: String,
}

pub struct GraphicalFrontendInternal{
    metric_backend: Backend,

    window : glutin::WindowedContext<glutin::PossiblyCurrent>,

    gl_context : glow::Context,

    egui : EguiGlow,

    counter : usize,

    selection : usize
}

pub struct GraphicalFrontend {

    event_loop : EventLoop<()>,

    internal : GraphicalFrontendInternal
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

        let (gl_window, context ) = GraphicalFrontend::create_display(&event_loop);

        let egui = egui_glow::EguiGlow::new(&gl_window, &context);

        let event_proxy = event_loop.create_proxy();

        metric_backend.add_callback(Box::new(move ||{
            event_proxy.send_event(());
        }));

        Ok(GraphicalFrontend {
            event_loop,
            internal : GraphicalFrontendInternal {
                metric_backend,
                window: gl_window,
                gl_context: context,
                egui,
                counter : 0,
                selection : 0
            }
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
            .with_title("egui_glow example");

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
        let mut clear_color = [0.1, 0.1, 0.1];


        let mut quit = false;

            let needs_repaint = self.egui.run(self.window.window(), |egui_ctx| {
                egui::SidePanel::left("my_side_panel").show(egui_ctx, |ui| {
                    ui.heading("Hello World!");
                    ui.label(format!("counter = {}", self.counter));
                    if ui.button("Quit").clicked() {
                        quit = true;
                    }
                });

                egui::CentralPanel::default().show(&egui_ctx, |ui| {
                    ui.vertical_centered_justified(|ui|{
                        let metric_data = self.metric_backend.map_metrics(|m|{
                            (m.get_label().to_string(), m.get_value().to_string(), m.get_unit().to_string())
                        });
                        ui.columns(3, |columns|{
                            let mut metric_id = 0;
                            for m in metric_data {
                                let c = columns[0].selectable_label(self.selection == metric_id,m.0);
                                columns[1].selectable_label(self.selection == metric_id,m.1);
                                columns[2].selectable_label(self.selection == metric_id,m.2);

                                if c.clicked() {
                                    self.selection = metric_id;
                                }

                                metric_id += 1;
                            }
                        });
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

                // draw things behind egui here

                self.egui.paint(&self.window, &self.gl_context, needs_repaint.1);

                self.counter += 1;

                // draw things on top of egui here

                self.window.swap_buffers().unwrap();
            }
    }
}

impl MetricFrontend for GraphicalFrontend {
    fn run(mut self) -> Result<(), Box<dyn Error>> {
        let mut internal = self.internal;

        self.event_loop.run_return( move |event, _, control_flow| {
            internal.event_handle(event, control_flow);
        });


        Ok(())
    }
}
