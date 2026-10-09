//! Stable candidates for swept projectile collision. Cell contents are indices;
//! sort before use so HashMap iteration order never influences combat or RNG.
use glam::Vec2;
use std::collections::HashMap;
pub struct Grid {
    cells: HashMap<(i32, i32), Vec<usize>>,
}
impl Grid {
    pub fn new(points: impl Iterator<Item = (usize, Vec2)>) -> Self {
        let mut cells: HashMap<_, Vec<usize>> = HashMap::new();
        for (i, p) in points {
            cells
                .entry(((p.x / 2.0).floor() as i32, (p.y / 2.0).floor() as i32))
                .or_default()
                .push(i);
        }
        Self { cells }
    }
    pub fn segment(&self, a: Vec2, b: Vec2, r: f32) -> Vec<usize> {
        let lo = (a.min(b) - Vec2::splat(r)) / 2.0;
        let hi = (a.max(b) + Vec2::splat(r)) / 2.0;
        let mut out = Vec::new();
        for y in lo.y.floor() as i32..=hi.y.floor() as i32 {
            for x in lo.x.floor() as i32..=hi.x.floor() as i32 {
                if let Some(ids) = self.cells.get(&(x, y)) {
                    out.extend_from_slice(ids);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}
