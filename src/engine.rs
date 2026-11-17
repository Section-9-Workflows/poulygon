// SPDX-License-Identifier: MIT
pub type Vec3 = [f32; 3];
pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn mul(a: Vec3, k: f32) -> Vec3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
pub fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let u = sub(b, a);
    let v = sub(c, a);
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    mul(n, 1.0 / dot(n, n).sqrt().max(1e-12))
}
pub struct Rotation {
    sx: f32,
    cx: f32,
    sy: f32,
    cy: f32,
}
impl Rotation {
    pub fn new(x: f32, y: f32) -> Self {
        let (sx, cx) = x.sin_cos();
        let (sy, cy) = y.sin_cos();
        Self { sx, cx, sy, cy }
    }
    pub fn apply(&self, v: Vec3) -> Vec3 {
        let x = self.cy * v[0] + self.sy * v[2];
        let z = -self.sy * v[0] + self.cy * v[2];
        [
            x,
            self.cx * v[1] - self.sx * z,
            self.sx * v[1] + self.cx * z,
        ]
    }
}
pub struct Camera {
    pub aspect: f32,
}
impl Camera {
    pub fn project(&self, p: Vec3) -> [f32; 4] {
        let distance = 5.0 - p[2];
        [
            p[0] * 2.4 / self.aspect,
            p[1] * 2.4,
            (21.0 * distance - 40.0) / 19.0,
            distance,
        ]
    }
}
pub const STEP: f64 = 1.0 / 120.0;
pub struct Clock {
    pub tick: u64,
    remainder: f64,
}
impl Clock {
    pub fn new() -> Self {
        Self {
            tick: 0,
            remainder: 0.0,
        }
    }
    pub fn advance(&mut self, elapsed: f64, mut update: impl FnMut(f32)) {
        self.remainder += elapsed;
        while self.remainder >= STEP {
            update(STEP as f32);
            self.tick += 1;
            self.remainder -= STEP;
        }
    }
    pub fn time(&self) -> f32 {
        (self.tick as f64 * STEP + self.remainder) as f32
    }
}
#[derive(Clone, Copy)]
pub struct Face {
    pub points: [Vec3; 3],
    pub normal: Vec3,
}
pub struct Mesh {
    pub faces: Vec<Face>,
}
impl Mesh {
    pub fn sphere(detail: usize) -> Self {
        let rows = detail + 2;
        let cols = rows * 2;
        let point = |r: usize, c: usize| {
            let a = std::f32::consts::PI * r as f32 / rows as f32;
            let b = std::f32::consts::TAU * c as f32 / cols as f32;
            [a.sin() * b.cos(), a.cos(), a.sin() * b.sin()]
        };
        let mut faces = Vec::with_capacity(2 * cols * (rows - 1));
        for r in 0..rows {
            for c in 0..cols {
                let a = point(r, c);
                let b = point(r + 1, c);
                let d = point(r, c + 1);
                let e = point(r + 1, c + 1);
                for (valid, mut points) in [(r > 0, [a, b, d]), (r + 1 < rows, [d, b, e])] {
                    if !valid {
                        continue;
                    }
                    let mut n = normal(points[0], points[1], points[2]);
                    if dot(n, points[0]) < 0.0 {
                        points.swap(1, 2);
                        n = mul(n, -1.0);
                    }
                    faces.push(Face { points, normal: n });
                }
            }
        }
        Self { faces }
    }
}
