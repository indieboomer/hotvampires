//! Rounded swept footprint, physical ledge handling and bounded ground snapping.
use crate::{
    sim::Wall,
    terrain::{Physics, Terrain},
};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Body {
    pub y: f32,
    pub prev_y: f32,
    pub velocity: f32,
    pub grounded: bool,
    pub visual_y: f32,
    pub stalled: f32,
}
impl Body {
    pub fn at(y: f32) -> Self {
        Self {
            y,
            prev_y: y,
            visual_y: y,
            grounded: true,
            ..Self::default()
        }
    }
}
pub fn support(t: &Terrain, p: Vec2, r: f32) -> f32 {
    let mut h = t.height(p);
    let lo = ((p - Vec2::splat(r) + Vec2::splat(crate::terrain::HALF)) / crate::terrain::XZ)
        .floor()
        .as_ivec2();
    let hi = ((p + Vec2::splat(r) + Vec2::splat(crate::terrain::HALF)) / crate::terrain::XZ)
        .floor()
        .as_ivec2();
    for z in lo.y.max(0)..=hi.y.min(crate::terrain::COLS as i32 - 1) {
        for x in lo.x.max(0)..=hi.x.min(crate::terrain::COLS as i32 - 1) {
            let center = Terrain::column_center(x as usize, z as usize);
            let d = ((p - center).abs() - Vec2::splat(crate::terrain::XZ * 0.5)).max(Vec2::ZERO);
            if d.length_squared() < r * r {
                // Exact lower hemisphere contact with a voxel's top corner.
                // A quarter-metre stair rises continuously over the rounded foot.
                h = h.max(t.top(x as usize, z as usize) - r + (r * r - d.length_squared()).sqrt());
            }
        }
    }
    h
}
/// Soil plus actual surviving structure tops below the feet (for falling/short steps).
pub fn ground(t: &Terrain, walls: &[Wall], p: Vec2, r: f32, ceiling: f32) -> f32 {
    let mut h = support(t, p, r);
    for w in walls {
        if p.x < w.origin.x - r
            || p.y < w.origin.z - r
            || p.x > w.origin.x + w.size[0] as f32 * crate::sim::CELL + r
            || p.y > w.origin.z + w.size[2] as f32 * crate::sim::CELL + r
        {
            continue;
        }
        for (i, &v) in w.cells.iter().enumerate() {
            if !v {
                continue;
            }
            let q = w.center(i);
            let top = q.y + crate::sim::CELL * 0.5;
            if top <= h {
                continue;
            }
            let d = ((p - Vec2::new(q.x, q.z)).abs() - Vec2::splat(crate::sim::CELL * 0.5))
                .max(Vec2::ZERO);
            if d.length_squared() < r * r {
                let contact = top - r + (r * r - d.length_squared()).sqrt();
                if contact <= ceiling {
                    h = h.max(contact);
                }
            }
        }
    }
    h
}
pub fn clear(t: &Terrain, walls: &[Wall], p: Vec2, y: f32, c: &Physics) -> bool {
    if !t.bounds(p, c.radius) {
        return false;
    }
    // Circular footprint versus voxel boxes, rather than a snag-prone square.
    for w in walls {
        let lo = Vec2::new(w.origin.x, w.origin.z) - Vec2::splat(c.radius);
        let hi = Vec2::new(
            w.origin.x + w.size[0] as f32 * crate::sim::CELL,
            w.origin.z + w.size[2] as f32 * crate::sim::CELL,
        ) + Vec2::splat(c.radius);
        if p.x < lo.x
            || p.y < lo.y
            || p.x > hi.x
            || p.y > hi.y
            || y > w.origin.y + w.size[1] as f32 * crate::sim::CELL
            || y + c.body_height < w.origin.y
        {
            continue;
        }
        for (i, &v) in w.cells.iter().enumerate() {
            if !v {
                continue;
            }
            let q = w.center(i);
            let d = (p - Vec2::new(q.x, q.z)).abs() - Vec2::splat(crate::sim::CELL * 0.5);
            let low = y + c.radius;
            let high = y + c.body_height - c.radius;
            let top = q.y + crate::sim::CELL * 0.5;
            let bottom = q.y - crate::sim::CELL * 0.5;
            let vertical = if top < low {
                low - top
            } else if bottom > high {
                bottom - high
            } else {
                0.0
            };
            if d.max(Vec2::ZERO).length_squared() + vertical * vertical
                < c.radius * c.radius - 0.00001
            {
                return false;
            }
        }
    }
    support(t, p, c.radius) <= y + 0.025
}
pub fn advance(
    t: &Terrain,
    walls: &[Wall],
    c: &Physics,
    p: &mut Vec2,
    b: &mut Body,
    delta: Vec2,
    dt: f32,
) {
    b.prev_y = b.y;
    let start = *p;
    let initial = ground(t, walls, *p, c.radius, b.y + 0.025);
    if b.y - initial > c.ground_snap || initial > b.y + 0.03 {
        b.grounded = false;
    }
    let steps = (delta.length() / 0.08).ceil().max(1.0) as usize;
    for _ in 0..steps {
        let d = delta / steps as f32;
        let try_move = |q: Vec2, b: &mut Body| -> bool {
            let h = ground(
                t,
                walls,
                q,
                c.radius,
                b.y + if b.grounded { c.step_height } else { 0.025 },
            );
            let rise = h - b.y;
            let lift = if b.grounded && rise > 0.0 && rise <= c.step_height {
                h
            } else {
                b.y
            };
            // Probe half a metre forward: stairs are slopes, substantial walls stay walls.
            let dir = (q - *p).normalize_or_zero();
            let grade = (ground(
                t,
                walls,
                *p + dir * 0.5,
                c.radius,
                b.y + c.step_height + c.radius,
            ) - initial)
                .abs()
                / 0.5;
            if lift > b.y && grade > c.max_slope_degrees.to_radians().tan() {
                return false;
            }
            if !clear(t, walls, q, lift, c) {
                return false;
            }
            if b.grounded {
                b.y = if rise >= 0.0 {
                    lift
                } else if -rise <= c.ground_snap {
                    h
                } else {
                    b.grounded = false;
                    b.y
                };
            }
            true
        };
        let grade = (ground(
            t,
            walls,
            *p + d.normalize_or_zero() * 0.5,
            c.radius,
            b.y + c.step_height + c.radius,
        ) - ground(t, walls, *p, c.radius, b.y + 0.025))
        .abs()
            / 0.5;
        let d = if b.grounded && grade <= c.max_slope_degrees.to_radians().tan() {
            d / (1.0 + grade * grade).sqrt()
        } else {
            d
        };
        if try_move(*p + d, b) {
            *p += d;
        } else if try_move(*p + Vec2::new(d.x, 0.0), b) {
            p.x += d.x;
        } else if try_move(*p + Vec2::new(0.0, d.y), b) {
            p.y += d.y;
        }
    }
    let floor = ground(t, walls, *p, c.radius, b.y + 0.025);
    if !b.grounded {
        b.velocity -= c.gravity * dt;
        let next = b.y + b.velocity * dt;
        if next <= floor && floor <= b.y + 0.03 && clear(t, walls, *p, floor, c) {
            b.y = floor;
            b.velocity = 0.0;
            b.grounded = true;
        } else {
            b.y = next;
        }
    } else {
        b.velocity = 0.0;
    }
    // Cosmetic correction is bounded to less than half a vertical voxel.
    b.visual_y += (b.y - b.visual_y) * (1.0 - (-dt / 0.09).exp());
    b.visual_y = b.visual_y.clamp(b.y - 0.1, b.y + 0.1);
    if delta.length_squared() > 0.000001 && p.distance_squared(start) < 0.0000001 {
        b.stalled += dt;
    } else {
        b.stalled = 0.0;
    }
}
pub fn point(p: Vec2, y: f32) -> Vec3 {
    Vec3::new(p.x, y, p.y)
}
/// Resolve support in the same authoritative edit tick, including shallow settling.
pub fn reconcile(t: &Terrain, walls: &[Wall], p: Vec2, c: &Physics, b: &mut Body) -> bool {
    let drop = b.y - ground(t, walls, p, c.radius, b.y + 0.025);
    if drop <= 0.025 || !b.grounded {
        return false;
    }
    if drop <= c.ground_snap {
        b.y -= drop;
        b.visual_y = b.visual_y.clamp(b.y - 0.1, b.y + 0.1);
        false
    } else {
        b.grounded = false;
        true
    }
}
