use crate::sim::Game;
use glam::{Vec2, Vec3};
#[derive(Default)]
pub struct Camera {
    pub follow: Option<f32>,
    pub fade: f32,
    pub clear_time: f32,
    pub jitter: f32,
}
impl Camera {
    pub fn update(&mut self, g: &Game, p: Vec2, yaw: f32, dt: f32) -> (Vec3, Vec3) {
        let desired = g.player_body.visual_y;
        let last = self.follow.unwrap_or(desired);
        let y = (last + (desired - last) * (1.0 - (-dt / g.physics.camera_follow_seconds).exp()))
            .clamp(desired - 1.0, desired + 1.0);
        self.jitter = self.jitter.max((y - last).abs());
        self.follow = Some(y);
        let center = Vec3::new(p.x, y + 0.6, p.y);
        let elevation = 30.0_f32.to_radians();
        let eye = center
            + Vec3::new(
                yaw.sin() * 31.0 * elevation.cos(),
                31.0 * elevation.sin(),
                yaw.cos() * 31.0 * elevation.cos(),
            );
        let side = Vec3::new(yaw.cos(), 0.0, -yaw.sin()) * 0.3;
        let foot = Vec3::new(p.x, g.player_body.y, p.y);
        let hidden = [
            foot + Vec3::Y * 0.2,
            foot + Vec3::Y * 0.8,
            foot + Vec3::Y * 1.4,
            foot + Vec3::Y * 0.8 + side,
            foot + Vec3::Y * 0.8 - side,
        ]
        .iter()
        .any(|&q| g.cover_ray3(eye, q).is_some());
        if hidden {
            self.clear_time = 0.0;
        } else {
            self.clear_time += dt;
        }
        let target = if hidden || (self.fade > 0.01 && self.clear_time < 0.15) {
            1.0
        } else {
            0.0
        };
        self.fade += (target - self.fade) * (1.0 - (-dt / g.physics.camera_fade_seconds).exp());
        (center, eye)
    }
}
