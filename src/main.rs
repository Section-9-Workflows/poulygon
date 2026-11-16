// SPDX-License-Identifier: MIT
// Copyright 2026 DoubleDensity <doubledense@gmail.com>
use std::ffi::c_void;
use std::time::Instant;

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex { position: [f32; 4], color: [f32; 4] }
extern "C" {
    fn facet_open(width: *mut u32, height: *mut u32) -> *mut c_void;
    fn facet_frame(display: *mut c_void, vertices: *const Vertex, count: u32) -> i32;
    fn facet_close(display: *mut c_void) -> i32;
    fn facet_cpu_seconds() -> f64;
}

const STEP: f64 = 1.0 / 120.0;
const CORNERS: [[f32; 3]; 8] = [
    [-1., -1., -1.], [1., -1., -1.], [1., 1., -1.], [-1., 1., -1.],
    [-1., -1., 1.], [1., -1., 1.], [1., 1., 1.], [-1., 1., 1.],
];
const FACES: [[usize; 4]; 6] = [
    [0, 3, 2, 1], [4, 5, 6, 7], [0, 4, 7, 3],
    [1, 2, 6, 5], [0, 1, 5, 4], [3, 7, 6, 2],
];
const NORMALS: [[f32; 3]; 6] = [
    [0., 0., -1.], [0., 0., 1.], [-1., 0., 0.],
    [1., 0., 0.], [0., -1., 0.], [0., 1., 0.],
];
const COLORS: [[f32; 3]; 6] = [
    [0.95, 0.22, 0.52], [0.16, 0.83, 0.95], [0.95, 0.53, 0.12],
    [0.58, 0.32, 0.95], [0.30, 0.86, 0.42], [0.95, 0.85, 0.45],
];

// Simulation advances on numbered ticks; rendering may run at a different rate.
struct Simulation { tick: u64, remainder: f64 }
impl Simulation {
    fn advance(&mut self, elapsed: f64) {
        self.remainder += elapsed;
        let steps = (self.remainder / STEP).floor() as u64;
        self.tick += steps;
        self.remainder -= steps as f64 * STEP;
    }
    fn time(&self) -> f32 { (self.tick as f64 * STEP + self.remainder) as f32 }
}

fn cube(time: f32, aspect: f32, out: &mut [Vertex; 36]) -> usize {
    let (sy, cy) = (time * 0.55).sin_cos();
    let (sx, cx) = (time * 0.31).sin_cos();
    let rotate = |v: [f32; 3]| {
        let x = cy * v[0] + sy * v[2];
        let z = -sy * v[0] + cy * v[2];
        [x, cx * v[1] - sx * z, sx * v[1] + cx * z]
    };
    let mut clip = [[0.; 4]; 8];
    for (i, point) in CORNERS.iter().enumerate() {
        let p = rotate(*point);
        let distance = 5.0 - p[2];
        // Perspective with near=1, far=20. The GPU clips and maps to the viewport.
        clip[i] = [p[0] * 2.4 / aspect, p[1] * 2.4,
                   (21.0 * distance - 40.0) / 19.0, distance];
    }
    let mut count = 0;
    for face in 0..6 {
        let n = rotate(NORMALS[face]);
        // The camera is at (0, 0, 5); each cube face lies one unit from its center.
        if 5.0 * n[2] <= 1.0 { continue; }
        let light = 0.24 + 0.76 * (n[0] * -0.36 + n[1] * 0.48 + n[2] * 0.8).max(0.);
        let rgb = COLORS[face];
        let color = [rgb[0] * light, rgb[1] * light, rgb[2] * light, 1.];
        for (j, corner) in [0, 1, 2, 0, 2, 3].iter().enumerate() {
            out[count + j] = Vertex { position: clip[FACES[face][*corner]], color };
        }
        count += 6;
    }
    count
}

fn main() {
    let (mut width, mut height) = (0, 0);
    let display = unsafe { facet_open(&mut width, &mut height) };
    if display.is_null() { std::process::exit(1); }
    let mut vertices = [Vertex { position: [0.; 4], color: [0.; 4] }; 36];
    let mut sim = Simulation { tick: 0, remainder: 0. };
    let start = Instant::now();
    let mut previous = start;
    let mut frames = 0u64;
    let mut report = start;
    let mut report_frames = 0;
    let mut cpu = unsafe { facet_cpu_seconds() };
    let failed = loop {
        let now = Instant::now();
        sim.advance(now.duration_since(previous).as_secs_f64());
        previous = now;
        let count = cube(sim.time(), width as f32 / height as f32, &mut vertices);
        let result = unsafe { facet_frame(display, vertices.as_ptr(), count as u32) };
        if result <= 0 { break result < 0; }
        frames += 1;
        let now = Instant::now();
        let seconds = now.duration_since(report).as_secs_f64();
        if seconds >= 5.0 {
            let new_cpu = unsafe { facet_cpu_seconds() };
            eprintln!("facet: {:.1} fps, {:.1}% CPU, {} frames, tick {}",
                (frames - report_frames) as f64 / seconds, (new_cpu - cpu) * 100.0 / seconds,
                frames, sim.tick);
            cpu = new_cpu;
            report = now;
            report_frames = frames;
        }
    };
    let cleanup = unsafe { facet_close(display) };
    eprintln!("facet: {} frames in {:.2}s", frames, start.elapsed().as_secs_f64());
    if failed || cleanup != 0 { std::process::exit(1); }
}
