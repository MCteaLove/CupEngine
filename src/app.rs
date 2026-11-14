use std::sync::Arc;
use std::time::Instant;

use glam::Vec2;
use winit::{
    dpi::LogicalSize,
    event::{DeviceEvent, ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::PhysicalKey,
    window::{Window, WindowBuilder},
};

use crate::input::Input;
use crate::renderer::{GuiPaint, Renderer};

pub struct AppConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    /// Частота фиксированного шага симуляции (Гц).
    pub fixed_hz: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self { title: "CupEngine".into(), width: 1280, height: 720, fixed_hz: 120.0 }
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Time {
    pub dt: f32,
    pub fixed_dt: f32,
    pub elapsed: f32,
    pub fps: f32,
}

pub struct Context<'a> {
    pub input: &'a Input,
    pub renderer: &'a mut Renderer,
    pub time: &'a Time,
    pub window: &'a Window,
}

pub trait Game {
    /// Один раз при старте.
    fn init(&mut self, _ctx: &mut Context) {}
    /// Фиксированный шаг (`ctx.time.fixed_dt`).
    fn fixed_update(&mut self, _ctx: &mut Context) {}
    /// Построение UI (egui). Здесь же задаётся область 3D-окна через `ctx.renderer.set_viewport`.
    fn ui(&mut self, _egui: &egui::Context, _ctx: &mut Context) {}
    /// Каждый кадр: камера и отправка draw-вызовов.
    fn frame(&mut self, ctx: &mut Context);
}

pub fn run<G: Game + 'static>(config: AppConfig, mut game: G) {
    crate::console::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let window = Arc::new(
        WindowBuilder::new()
            .with_title(config.title.clone())
            .with_inner_size(LogicalSize::new(config.width, config.height))
            .build(&event_loop)
            .unwrap(),
    );

    let mut renderer = pollster::block_on(Renderer::new(window.clone()));
    let mut input = Input::default();
    let mut time = Time { fixed_dt: 1.0 / config.fixed_hz, fps: 60.0, ..Default::default() };

    let gui_ctx = egui::Context::default();
    let mut gui_state = egui_winit::State::new(
        gui_ctx.clone(),
        egui::ViewportId::ROOT,
        window.as_ref(),
        Some(window.scale_factor() as f32),
        None,
    );

    {
        let mut ctx = Context { input: &input, renderer: &mut renderer, time: &time, window: &window };
        game.init(&mut ctx);
    }

    let mut last = Instant::now();
    let mut acc = 0.0f32;

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { event, .. } => {
                let resp = gui_state.on_window_event(&window, &event);
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(s) => renderer.resize(s.width, s.height),
                    WindowEvent::KeyboardInput {
                        event: KeyEvent { physical_key: PhysicalKey::Code(code), state, .. },
                        ..
                    } => {
                        // отпускание клавиш пропускаем всегда, чтобы они не "залипали"
                        if !resp.consumed || state == ElementState::Released {
                            input.set_key(code, state == ElementState::Pressed);
                        }
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        input.set_mouse(button, state == ElementState::Pressed)
                    }
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = (now - last).as_secs_f32().min(0.1);
                        last = now;
                        time.dt = dt;
                        time.elapsed += dt;
                        time.fps = time.fps * 0.95 + (1.0 / dt.max(1e-6)) * 0.05;
                        acc += dt;

                        let raw_input = gui_state.take_egui_input(&window);
                        {
                            let mut ctx = Context {
                                input: &input,
                                renderer: &mut renderer,
                                time: &time,
                                window: &window,
                            };
                            while acc >= time.fixed_dt {
                                game.fixed_update(&mut ctx);
                                acc -= time.fixed_dt;
                            }

                            let full = gui_ctx.run(raw_input, |e| game.ui(e, &mut ctx));
                            game.frame(&mut ctx);

                            let paint = GuiPaint {
                                jobs: gui_ctx.tessellate(full.shapes, full.pixels_per_point),
                                textures_delta: full.textures_delta,
                                pixels_per_point: full.pixels_per_point,
                            };
                            ctx.renderer.render_frame(Some(paint));
                            gui_state.handle_platform_output(&window, full.platform_output);
                        }
                        input.end_frame();
                    }
                    _ => {}
                }
            }
            Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } => {
                input.add_mouse_delta(Vec2::new(delta.0 as f32, delta.1 as f32));
            }
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        })
        .unwrap();
}
