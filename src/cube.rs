// SPDX-License-Identifier: MIT
use crate::platform::Vertex;
const CORNERS: [[f32; 3]; 8] = [
    [-1., -1., -1.],
    [1., -1., -1.],
    [1., 1., -1.],
    [-1., 1., -1.],
    [-1., -1., 1.],
    [1., -1., 1.],
    [1., 1., 1.],
    [-1., 1., 1.],
];
const FACES: [[usize; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 4, 7, 3],
    [1, 2, 6, 5],
    [0, 1, 5, 4],
    [3, 7, 6, 2],
];
const NORMALS: [[f32; 3]; 6] = [
    [0., 0., -1.],
    [0., 0., 1.],
    [-1., 0., 0.],
    [1., 0., 0.],
    [0., -1., 0.],
    [0., 1., 0.],
];
const COLORS: [[f32; 3]; 6] = [
    [0.95, 0.22, 0.52],
    [0.16, 0.83, 0.95],
    [0.95, 0.53, 0.12],
    [0.58, 0.32, 0.95],
    [0.30, 0.86, 0.42],
    [0.95, 0.85, 0.45],
];

pub fn draw(time: f32, aspect: f32, out: &mut [Vertex; 36]) -> usize {
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
        clip[i] = [
            p[0] * 2.4 / aspect,
            p[1] * 2.4,
            (21.0 * distance - 40.0) / 19.0,
            distance,
        ];
    }
    let mut count = 0;
    for face in 0..6 {
        let n = rotate(NORMALS[face]);
        // The camera is at (0, 0, 5); each cube face lies one unit from its center.
        if 5.0 * n[2] <= 1.0 {
            continue;
        }
        let light = 0.24 + 0.76 * (n[0] * -0.36 + n[1] * 0.48 + n[2] * 0.8).max(0.);
        let rgb = COLORS[face];
        let color = [rgb[0] * light, rgb[1] * light, rgb[2] * light, 1.];
        for (j, corner) in [0, 1, 2, 0, 2, 3].iter().enumerate() {
            out[count + j] = Vertex {
                position: clip[FACES[face][*corner]],
                color,
            };
        }
        count += 6;
    }
    count
}
