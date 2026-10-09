mod content;
mod playtest;
mod render;
mod shop;
mod sim;
mod spatial;
#[cfg(test)]
mod tests;
mod ui;

use glam::Vec2;
use std::{collections::HashSet, time::Instant};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
struct App {
    window: Option<Window>,
    renderer: Option<render::Renderer>,
    ctx: egui::Context,
    gui: Option<egui_winit::State>,
    game: sim::Game,
    keys: HashSet<KeyCode>,
    yaw: f32,
    zoom: f32,
    paused: bool,
    focused: bool,
    last: Instant,
    notice: String,
    mouse_middle: bool,
    cursor: f64,
    bot: bool,
    bot_input: Vec2,
    bot_frame: u32,
    frames: u32,
    limit: Option<u32>,
    scene: render::SceneCache,
    replay: Option<playtest::Replay>,
    ui_smoke: bool,
}
impl App {
    fn new(bot: bool, limit: Option<u32>) -> Self {
        Self {
            window: None,
            renderer: None,
            ctx: egui::Context::default(),
            gui: None,
            game: sim::Game::new(11, content::Config::default()),
            keys: HashSet::new(),
            yaw: 0.4,
            zoom: 10.0,
            paused: false,
            focused: true,
            last: Instant::now(),
            notice: String::new(),
            mouse_middle: false,
            cursor: 0.0,
            bot,
            bot_input: Vec2::ZERO,
            bot_frame: 0,
            frames: 0,
            limit,
            scene: render::SceneCache::default(),
            replay: None,
            ui_smoke: false,
        }
    }
    fn redraw(&mut self, el: &ActiveEventLoop) {
        let window = self.window.as_ref().unwrap();
        if window.inner_size().width == 0 || window.inner_size().height == 0 {
            self.last = Instant::now();
            return;
        }
        let now = Instant::now();
        let real_dt = now.duration_since(self.last).as_secs_f32().min(0.1);
        self.last = now;
        let dt = if self.bot { 1.0 / 60.0 } else { real_dt };
        let was_shop = self.game.phase == sim::Phase::Shop;
        if let Some(replay) = &mut self.replay {
            if !self.paused {
                std::mem::swap(&mut self.game, &mut replay.game);
                let result = replay.step();
                let finished = replay.finished();
                std::mem::swap(&mut self.game, &mut replay.game);
                if let Err(e) = result {
                    eprintln!("Replay failed: {e}");
                    el.exit();
                }
                if finished {
                    println!(
                        "Rendered replay finished: {:?}, kills {}, purchases {}",
                        self.game.phase, self.game.metrics.kills, self.game.metrics.purchases
                    );
                    el.exit();
                }
            }
        } else if self.focused && !self.paused {
            let key = |k| if self.keys.contains(&k) { 1.0 } else { 0.0 };
            self.yaw += (key(KeyCode::KeyE) - key(KeyCode::KeyQ)) * dt * 1.8;
            let input = if self.bot {
                if self.game.phase == sim::Phase::Shop {
                    playtest::shop_actions(&mut self.game, "mobile");
                }
                if self.bot_frame.is_multiple_of(9) {
                    self.bot_input = playtest::movement(&self.game, "mobile");
                }
                self.bot_frame += 1;
                self.bot_input
            } else {
                let local = Vec2::new(
                    key(KeyCode::KeyD) - key(KeyCode::KeyA),
                    key(KeyCode::KeyS) - key(KeyCode::KeyW),
                )
                .clamp_length_max(1.0);
                let right = Vec2::new(self.yaw.cos(), -self.yaw.sin());
                let down = Vec2::new(self.yaw.sin(), self.yaw.cos());
                right * local.x + down * local.y
            };
            self.game.frame(dt, input);
        }
        if !was_shop
            && self.game.phase == sim::Phase::Shop
            && let Err(e) = self
                .game
                .save(std::path::Path::new("saves/checkpoint.json"))
        {
            self.notice = e;
        }
        let mut input = self.gui.as_mut().unwrap().take_egui_input(window);
        if self.ui_smoke && [60, 61, 80, 81].contains(&self.frames) {
            let id = if self.frames < 70 {
                "smoke-buy"
            } else {
                "smoke-start"
            };
            if let Some(rect) = self
                .ctx
                .data(|data| data.get_temp::<egui::Rect>(egui::Id::new(id)))
            {
                input.focused = true;
                input.events.push(egui::Event::PointerMoved(rect.center()));
                input.events.push(egui::Event::PointerButton {
                    pos: rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed: self.frames.is_multiple_of(2),
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
        let renderer = self.renderer.as_mut().unwrap();
        let output = self.ctx.run(input, |ctx| {
            ui::draw(
                ctx,
                &mut self.game,
                &mut self.paused,
                &mut self.notice,
                1.0 / real_dt.max(0.001),
                &renderer.adapter,
            )
        });
        self.gui
            .as_mut()
            .unwrap()
            .handle_platform_output(window, output.platform_output.clone());
        let vertices = self.scene.scene(
            &self.game,
            self.yaw,
            self.zoom,
            renderer.extent.width as f32 / renderer.extent.height as f32,
        );
        match renderer.draw(&vertices, &self.ctx, output) {
            Ok(false) => {
                if let Err(e) = renderer.resize(window) {
                    eprintln!("Resize failed: {e}");
                    el.exit();
                }
            }
            Err(e) => {
                eprintln!("Render failed: {e}");
                el.exit();
            }
            _ => {}
        }
        // Bounded event retention in human sessions; headless mode writes each event.
        self.game.events.clear();
        if self.ui_smoke && self.frames == 90 {
            if self.game.level("light_frame") != 1
                || self.game.currency != 85
                || self.game.phase != sim::Phase::Combat
                || self.game.wave != 2
            {
                eprintln!(
                    "Rendered UI smoke FAILED: {:?}, wave {}, currency {}, level {}",
                    self.game.phase,
                    self.game.wave,
                    self.game.currency,
                    self.game.level("light_frame")
                );
                std::process::exit(1);
            }
            println!(
                "Rendered egui input smoke PASS: Light Frame purchased, 105 -> 85 coins, next-wave button started wave 2"
            );
            el.exit();
        }
        self.frames += 1;
        if self.limit.is_some_and(|n| self.frames >= n) {
            println!(
                "Rendered {} frames on {}; world {:.3}s; phase {:?}",
                self.frames, renderer.adapter, self.game.world, self.game.phase
            );
            el.exit();
        } else {
            window.request_redraw();
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = el
            .create_window(
                Window::default_attributes()
                    .with_title("Hot Vampires — Rust / Vulkan")
                    .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0)),
            )
            .unwrap();
        let renderer = match render::Renderer::new(&window) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{e}");
                el.exit();
                return;
            }
        };
        println!("Vulkan adapter: {}", renderer.adapter);
        self.ctx.set_visuals(egui::Visuals::dark());
        self.gui = Some(egui_winit::State::new(
            self.ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        ));
        self.renderer = Some(renderer);
        self.window = Some(window);
        self.last = Instant::now();
        self.window.as_ref().unwrap().request_redraw();
    }
    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let (Some(gui), Some(window)) = (&mut self.gui, &self.window) {
            let _ = gui.on_window_event(window, &event);
        }
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => self.redraw(el),
            WindowEvent::Resized(_) => {
                if let (Some(r), Some(w)) = (&mut self.renderer, &self.window)
                    && let Err(e) = r.resize(w)
                {
                    eprintln!("Resize: {e}");
                    el.exit();
                }
                self.last = Instant::now();
            }
            WindowEvent::Focused(f) => {
                self.focused = f;
                self.keys.clear();
                self.mouse_middle = false;
                self.last = Instant::now();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(k) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        self.keys.insert(k);
                        if !event.repeat {
                            match k {
                                KeyCode::Escape => self.paused = !self.paused,
                                KeyCode::F5 => match sim::Game::load(std::path::Path::new(
                                    "saves/checkpoint.json",
                                )) {
                                    Ok(g) => self.game = g,
                                    Err(e) => self.notice = e,
                                },
                                KeyCode::KeyR
                                    if matches!(
                                        self.game.phase,
                                        sim::Phase::Dead | sim::Phase::Victory
                                    ) =>
                                {
                                    self.game =
                                        sim::Game::new(self.game.seed, self.game.config.clone())
                                }
                                _ => {}
                            }
                        }
                    } else {
                        self.keys.remove(&k);
                    }
                }
            }
            WindowEvent::MouseInput {
                button: MouseButton::Middle,
                state,
                ..
            } => self.mouse_middle = state == ElementState::Pressed,
            WindowEvent::CursorMoved { position, .. } => {
                if self.mouse_middle {
                    self.yaw += (position.x - self.cursor) as f32 * 0.006;
                }
                self.cursor = position.x;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.03,
                };
                self.zoom = (self.zoom - amount).clamp(5.0, 16.0);
            }
            _ => {}
        }
    }
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let replay_path = if args.first().is_some_and(|a| a == "playtest")
        && args.get(1).is_some_and(|a| a == "replay")
    {
        args.iter()
            .position(|a| a == "--run")
            .and_then(|i| args.get(i + 1))
    } else {
        None
    };
    if args.first().is_some_and(|a| a == "playtest")
        && !(replay_path.is_some() && args.iter().any(|a| a == "--render"))
    {
        if let Some(path) = replay_path {
            if let Err(e) = playtest::replay_headless(std::path::Path::new(path)) {
                eprintln!("{e}");
                std::process::exit(1);
            }
            return;
        }
        let result=match args.get(1).map(String::as_str){Some("run")=>playtest::run(&args),Some("compare")=>playtest::compare(&args),Some("stress")=>playtest::stress(),_=>Err("Use playtest run [--config path] --output directory, playtest compare --baseline directory --candidate directory, playtest replay --run path [--render], or playtest stress".into())};
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    let limit = args
        .iter()
        .position(|a| a == "--frames")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok());
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new(args.iter().any(|a| a == "--bot"), limit);
    app.ui_smoke = args.iter().any(|a| a == "--ui-smoke");
    if let Some(path) = replay_path {
        match playtest::Replay::load(std::path::Path::new(path)) {
            Ok(replay) => {
                app.game = replay.game.clone();
                app.replay = Some(replay);
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
    if let Some(path) = args
        .iter()
        .position(|a| a == "--checkpoint")
        .and_then(|i| args.get(i + 1))
    {
        match sim::Game::load(std::path::Path::new(path)) {
            Ok(g) => app.game = g,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
    event_loop.run_app(&mut app).unwrap();
    // Destroy Vulkan surface before the native window, regardless of field drop order.
    app.renderer.take();
}
