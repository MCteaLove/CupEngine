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
use crate::renderer::Renderer;

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
    /// Длительность последнего кадра (сек).
    pub dt: f32,
    /// Шаг фиксированной симуляции (сек).
    pub fixed_dt: f32,
    /// Время с запуска (сек).
    pub elapsed: f32,
    pub fps: f32,
}

/// Всё, что движок даёт игре в каждом колбэке.
pub struct Context<'a> {
    pub input: &'a Input,
    pub renderer: &'a mut Renderer,
    pub time: &'a Time,
    pub window: &'a Window,
}

/// Реализуйте этот трейт для своей игры.
pub trait Game {
    /// Один раз при старте: создание мешей, загрузка ресурсов.
    fn init(&mut self, _ctx: &mut Context) {}
    /// Фиксированный шаг (`ctx.time.fixed_dt`): физика, логика, пули.
    fn fixed_update(&mut self, _ctx: &mut Context) {}
    /// Каждый кадр: ввод, камера, отправка draw-вызовов.
    fn frame(&mut self, ctx: &mut Context);
}

pub fn run<G: Game + 'static>(config: AppConfig, mut game: G) {
    env_logger::init();

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

    {
        let mut ctx = Context { input: &input, renderer: &mut renderer, time: &time, window: &window };
        game.init(&mut ctx);
    }

    let mut last = Instant::now();
    let mut acc = 0.0f32;

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(s) => renderer.resize(s.width, s.height),
                WindowEvent::KeyboardInput {
                    event: KeyEvent { physical_key: PhysicalKey::Code(code), state, .. },
                    ..
                } => input.set_key(code, state == ElementState::Pressed),
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
                        game.frame(&mut ctx);
                        ctx.renderer.render_frame();
                    }
                    input.end_frame();
                }
                _ => {}
            },
            Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } => {
                input.add_mouse_delta(Vec2::new(delta.0 as f32, delta.1 as f32));
            }
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        })
        .unwrap();
}
