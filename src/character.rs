// SPDX-License-Identifier: MIT
use crate::engine::*;
use std::f32::consts::TAU;

fn loft(profile: &[[f32; 4]], sides: usize, face: bool) -> Mesh {
    let mut rings = Vec::with_capacity(profile.len() * sides);
    for &[y, xr, zr, zc] in profile {
        for i in 0..sides {
            let a = TAU * i as f32 / sides as f32;
            let x = xr * a.sin();
            let mut z = zc + zr * a.cos();
            if a.cos() > 0. && face {
                let nose =
                    (-((x / 0.055).powi(2))).exp() * (1. - ((y - 0.01) / 0.14).abs()).max(0.);
                let eye =
                    (-(((x.abs() - 0.095) / 0.065).powi(2) + ((y - 0.07) / 0.05).powi(2))).exp();
                let brow =
                    (-(((x.abs() - 0.10) / 0.08).powi(2) + ((y - 0.14) / 0.04).powi(2))).exp();
                z += 0.095 * nose - 0.025 * eye + 0.025 * brow;
            }
            rings.push([x, y, z]);
        }
    }
    let mut faces = Vec::new();
    for r in 0..profile.len() - 1 {
        for c in 0..sides {
            let a = r * sides + c;
            let b = r * sides + (c + 1) % sides;
            let d = a + sides;
            let e = b + sides;
            for mut points in [
                [rings[a], rings[b], rings[d]],
                [rings[b], rings[e], rings[d]],
            ] {
                let mut n = normal(points[0], points[1], points[2]);
                let radial = [points[0][0], 0., points[0][2] - profile[r][3]];
                if dot(n, radial) < 0. {
                    points.swap(1, 2);
                    n = mul(n, -1.);
                }
                faces.push(Face { points, normal: n });
            }
        }
    }
    for r in [0, profile.len() - 1] {
        let center = [0., profile[r][0], profile[r][3]];
        for c in 0..sides {
            let mut points = [
                center,
                rings[r * sides + c],
                rings[r * sides + (c + 1) % sides],
            ];
            let mut n = normal(points[0], points[1], points[2]);
            if (r == 0 && n[1] > 0.) || (r != 0 && n[1] < 0.) {
                points.swap(1, 2);
                n = mul(n, -1.);
            }
            faces.push(Face { points, normal: n });
        }
    }
    Mesh { faces }
}
pub fn head() -> Mesh {
    loft(
        &[
            [-0.29, 0.07, 0.09, 0.045],
            [-0.24, 0.15, 0.14, 0.025],
            [-0.18, 0.19, 0.18, 0.01],
            [-0.10, 0.215, 0.205, 0.],
            [0., 0.23, 0.205, 0.],
            [0.07, 0.225, 0.195, -0.005],
            [0.14, 0.225, 0.195, -0.01],
            [0.23, 0.2, 0.19, -0.015],
            [0.30, 0.13, 0.14, -0.02],
            [0.335, 0.015, 0.02, -0.02],
        ],
        20,
        true,
    )
}
pub fn torso() -> Mesh {
    loft(
        &[
            [-0.42, 0.24, 0.18, 0.],
            [-0.20, 0.25, 0.16, 0.],
            [0.10, 0.28, 0.185, 0.],
            [0.35, 0.37, 0.23, 0.],
            [0.58, 0.45, 0.245, 0.],
            [0.72, 0.43, 0.205, 0.],
            [0.82, 0.27, 0.16, 0.],
            [0.90, 0.12, 0.12, 0.],
        ],
        16,
        false,
    )
}
pub fn bend_joint(start: Vec3, end: Vec3, upper: f32, lower: f32, hint: Vec3) -> (Vec3, Vec3) {
    let delta = sub(end, start);
    let length = dot(delta, delta).sqrt().max(1e-6);
    let axis = mul(delta, 1. / length);
    let distance = length.clamp((upper - lower).abs() + 0.001, upper + lower - 0.001);
    let along = (upper * upper - lower * lower + distance * distance) / (2. * distance);
    let height = (upper * upper - along * along).max(0.).sqrt();
    let bend = sub(hint, mul(axis, dot(hint, axis)));
    let bend = mul(bend, 1. / dot(bend, bend).sqrt().max(1e-6));
    (
        add(add(start, mul(axis, along)), mul(bend, height)),
        add(start, mul(axis, distance)),
    )
}
