// SPDX-License-Identifier: MIT
use crate::{engine::*, platform::Vertex};
use std::{
    collections::BTreeMap,
    f32::consts::{PI, TAU},
};

fn unit(v: Vec3) -> Vec3 {
    mul(v, 1. / dot(v, v).sqrt().max(1e-8))
}
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
struct Cell {
    center: Vec3,
    corners: Vec<Vec3>,
}
pub struct Cybersphere {
    cells: Vec<Cell>,
    targets: Vec<usize>,
    body: Mesh,
    node: Mesh,
}

impl Cybersphere {
    pub fn new() -> Self {
        let g = (1. + 5f32.sqrt()) / 2.;
        let mut points = vec![
            [-1., g, 0.],
            [1., g, 0.],
            [-1., -g, 0.],
            [1., -g, 0.],
            [0., -1., g],
            [0., 1., g],
            [0., -1., -g],
            [0., 1., -g],
            [g, 0., -1.],
            [g, 0., 1.],
            [-g, 0., -1.],
            [-g, 0., 1.],
        ];
        for p in &mut points {
            *p = unit(*p);
        }
        let mut faces = vec![
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ];
        for _ in 0..2 {
            let mut edges = BTreeMap::new();
            let mut divided = Vec::with_capacity(faces.len() * 4);
            for [a, b, c] in faces {
                let mut midpoint = |a: usize, b: usize| {
                    *edges.entry((a.min(b), a.max(b))).or_insert_with(|| {
                        let i = points.len();
                        points.push(unit(add(points[a], points[b])));
                        i
                    })
                };
                let ab = midpoint(a, b);
                let bc = midpoint(b, c);
                let ca = midpoint(c, a);
                divided.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
            }
            faces = divided;
        }
        // The dual of the subdivided icosahedron gives hexagons and twelve pentagons.
        let centers: Vec<_> = faces
            .iter()
            .map(|f| unit(add(add(points[f[0]], points[f[1]]), points[f[2]])))
            .collect();
        let cells: Vec<Cell> = points
            .iter()
            .enumerate()
            .map(|(i, &center)| {
                let tangent = unit(cross(
                    center,
                    if center[1].abs() < 0.9 {
                        [0., 1., 0.]
                    } else {
                        [1., 0., 0.]
                    },
                ));
                let bitangent = cross(center, tangent);
                let mut corners: Vec<_> = faces
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.contains(&i))
                    .map(|(j, _)| centers[j])
                    .collect();
                corners.sort_by(|a, b| {
                    dot(*a, bitangent)
                        .atan2(dot(*a, tangent))
                        .total_cmp(&dot(*b, bitangent).atan2(dot(*b, tangent)))
                });
                for p in &mut corners {
                    *p = mul(unit(add(mul(*p, 0.93), mul(center, 0.07))), 3.);
                }
                Cell {
                    center: mul(center, 3.),
                    corners,
                }
            })
            .collect();
        let mut targets: Vec<_> = cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.center[1] > 0.2 && c.center[1] < 1.5)
            .map(|(i, _)| i)
            .collect();
        targets.sort_by(|&a, &b| {
            cells[a].center[0]
                .atan2(cells[a].center[2])
                .total_cmp(&cells[b].center[0].atan2(cells[b].center[2]))
        });
        Self {
            cells,
            targets,
            body: Mesh::sphere(2),
            node: Mesh::solid(false),
        }
    }
    pub fn draw(&self, time: f32, camera: &Camera, out: &mut Vec<Vertex>) {
        out.clear();
        let angle = time * 0.13;
        let eye = [
            angle.sin() * 2.15,
            0.24 + 0.12 * (time * 0.19).sin(),
            angle.cos() * 2.15,
        ];
        let forward = unit(sub([0., 0.05, 0.], eye));
        let right = unit(cross(forward, [0., 1., 0.]));
        let view = View {
            camera,
            eye,
            forward,
            right,
            up: cross(right, forward),
            model: Rotation::new(0., 0.),
        };
        let step = (time / 7.) as usize % self.targets.len();
        let active = self.targets[(step * 5) % self.targets.len()];
        let previous = self.targets[((step + self.targets.len() - 1) * 5) % self.targets.len()];
        let target = self.cells[active].center;
        let before = self.cells[previous].center;
        let phase = (time / 7.).fract();
        let ease = (phase * 3.5).min(1.);
        let ease = ease * ease * (3. - 2. * ease);
        let previous_yaw = before[0].atan2(before[2]);
        let delta = (target[0].atan2(target[2]) - previous_yaw + PI).rem_euclid(TAU) - PI;
        let yaw = previous_yaw + delta * ease;
        let reach = (phase * PI).sin().max(0.);
        for (i, cell) in self.cells.iter().enumerate() {
            let center = cell.center;
            let pulse = (time * 1.1 - i as f32 * 0.39).cos().max(0.).powi(12);
            let rgb = if i == active {
                [0.85, 0.25 + reach * 0.35, 0.08]
            } else {
                [
                    0.05 + 0.4 * pulse,
                    0.22 * (1. - pulse),
                    0.4 * (1. - pulse) + 0.08,
                ]
            };
            for j in 0..cell.corners.len() {
                let a = cell.corners[j];
                let b = cell.corners[(j + 1) % cell.corners.len()];
                if i == active || pulse > 0.92 {
                    view.triangle([center, a, b], mul(rgb, 0.65), out);
                }
                view.line(a, b, rgb, 0.0025, out);
            }
        }
        {
            let axis = Rotation::new(0.45, time * 0.12);
            let point = |a: f32| axis.apply([2.75 * a.cos(), 2.75 * a.sin(), 0.]);
            let color = [0.08, 0.55, 0.75];
            for j in 0..64 {
                view.line(
                    point(TAU * j as f32 / 64.),
                    point(TAU * (j + 1) as f32 / 64.),
                    color,
                    0.003,
                    out,
                );
            }
            for j in 0..3 {
                let p = point(time * 0.5 + j as f32 * TAU / 3.);
                view.mesh(&self.node, p, [0.075; 3], [1., 0.8, 0.32], time, out);
            }
        }
        let world = view;
        let view = View {
            model: Rotation::new(0., yaw),
            ..view
        };
        let face = View {
            model: Rotation::new(0., yaw + (delta * (1. - ease)).clamp(-0.45, 0.45)),
            ..view
        };
        let gold = [0.91, 0.48, 0.1];
        let bob = (time * 0.7).sin() * 0.045;
        let head = [0., 1.13 + bob, 0.];
        face.mesh(&self.body, head, [0.29, 0.38, 0.27], gold, 0., out);
        view.mesh(
            &self.body,
            [0., 0.38 + bob, 0.],
            [0.43, 0.62, 0.24],
            gold,
            0.,
            out,
        );
        view.mesh(
            &self.body,
            [0., -0.3 + bob, 0.],
            [0.3, 0.29, 0.23],
            gold,
            0.,
            out,
        );
        for side in [-1., 1.] {
            let shoulder = [side * 0.42, 0.65 + bob, 0.];
            let elbow = if side > 0. {
                [0.6, 0.45 + bob + reach * 0.3, 0.2 + reach * 0.55]
            } else {
                [-0.75, 0.15 + bob, 0.2]
            };
            let hand = if side > 0. {
                [0.3, 0.55 + bob + reach * 0.4, 0.5 + reach * 1.25]
            } else {
                [-0.8, 0.05 + bob, 0.45 + reach * 0.2]
            };
            if side > 0. && reach > 0.45 {
                world.line(
                    view.model.apply(hand),
                    mul(target, 0.98),
                    [0.05, reach * 0.7, reach],
                    0.002,
                    out,
                );
            }
            view.limb(shoulder, elbow, 0.13, gold, out);
            view.limb(elbow, hand, 0.10, gold, out);
            view.mesh(&self.body, hand, [0.12, 0.15, 0.1], gold, 0., out);
            for finger in 0..4 {
                let base = add(hand, [side * (finger as f32 - 1.5) * 0.045, 0.1, 0.]);
                let curl = 0.035 + 0.045 * (time * 0.8 + finger as f32 * 0.3).sin();
                let joint = add(base, [side * (finger as f32 - 1.5) * 0.015, 0.09, curl]);
                let tip = add(joint, [0., 0.06, curl]);
                view.limb(base, joint, 0.022, gold, out);
                view.limb(joint, tip, 0.018, gold, out);
            }
            view.limb(
                add(hand, [-side * 0.09, 0., 0.]),
                add(hand, [-side * 0.17, 0.08, 0.04]),
                0.03,
                gold,
                out,
            );
            view.mesh(&self.body, elbow, [0.13; 3], gold, 0., out);
            let hip = [side * 0.17, -0.43 + bob, 0.];
            let knee = [side * 0.3, -1.02 + bob, 0.05];
            let foot = [side * 0.47, -1.65 + bob, 0.12];
            view.limb(hip, knee, 0.16, gold, out);
            view.limb(knee, foot, 0.11, gold, out);
            view.mesh(&self.body, foot, [0.13, 0.11, 0.22], gold, 0., out);
            let eye = [side * 0.105, 1.22 + bob, 0.245];
            face.mesh(
                &self.node,
                eye,
                [0.07, 0.035, 0.035],
                [0.25, 0.9, 1.],
                0.,
                out,
            );
            let circuit = [
                [side * 0.1, 0.85 + bob, 0.15],
                [side * 0.25, 0.65 + bob, 0.21],
                [side * 0.25, 0.25 + bob, 0.23],
                [side * 0.13, 0.1 + bob, 0.24],
                [side * 0.13, -0.15 + bob, 0.23],
            ];
            for pair in circuit.windows(2) {
                view.line(pair[0], pair[1], [1., 0.72, 0.24], 0.0035, out);
            }
            view.line(shoulder, elbow, [1., 0.66, 0.13], 0.003, out);
            view.line(elbow, hand, [1., 0.66, 0.13], 0.003, out);
        }
        face.triangle(
            [
                [-0.075, 1.04 + bob, 0.25],
                [0.075, 1.04 + bob, 0.25],
                [0., 0.96 + bob, 0.23],
            ],
            [0.06, 0.025, 0.015],
            out,
        );
        face.mesh(
            &self.node,
            [0., 1.15 + bob, 0.27],
            [0.045, 0.08, 0.065],
            gold,
            0.,
            out,
        );
        for i in 0..5 {
            let y = 0.98 + i as f32 * 0.095 + bob;
            let width = 0.24 * (1. - ((y - head[1]) / 0.43).powi(2)).max(0.).sqrt();
            face.line(
                [-width, y, 0.18],
                [0., y + 0.03, 0.275],
                [1., 0.61, 0.1],
                0.0018,
                out,
            );
            face.line(
                [0., y + 0.03, 0.275],
                [width, y, 0.18],
                [1., 0.61, 0.1],
                0.0018,
                out,
            );
        }
    }
}
#[derive(Clone, Copy)]
struct View<'a> {
    camera: &'a Camera,
    eye: Vec3,
    forward: Vec3,
    right: Vec3,
    up: Vec3,
    model: Rotation,
}
const NEAR: f32 = 0.18;
impl View<'_> {
    fn camera_point(&self, p: Vec3) -> Vec3 {
        let v = sub(self.model.apply(p), self.eye);
        [dot(v, self.right), dot(v, self.up), dot(v, self.forward)]
    }
    fn project(&self, p: Vec3) -> [f32; 4] {
        [
            p[0] * 1.15 / self.camera.aspect,
            p[1] * 1.15,
            (20. + NEAR) / (20. - NEAR) * p[2] - 40. * NEAR / (20. - NEAR),
            p[2],
        ]
    }
    fn plane(&self, p: Vec3, plane: usize) -> f32 {
        match plane {
            0 => p[2] - NEAR,
            1 => p[2] + p[0] * 1.15 / self.camera.aspect,
            2 => p[2] - p[0] * 1.15 / self.camera.aspect,
            3 => p[2] + p[1] * 1.15,
            _ => p[2] - p[1] * 1.15,
        }
    }
    fn triangle(&self, p: [Vec3; 3], rgb: Vec3, out: &mut Vec<Vertex>) {
        let mut input = [[0.; 3]; 8];
        for (i, p) in p.iter().enumerate() {
            input[i] = self.camera_point(*p);
        }
        let mut count = 3;
        // Bound the submitted geometry to the visible frustum before GPU clipping.
        for plane in 0..5 {
            let mut output = [[0.; 3]; 8];
            let mut next = 0;
            for i in 0..count {
                let a = input[i];
                let b = input[(i + 1) % count];
                let da = self.plane(a, plane);
                let db = self.plane(b, plane);
                if da >= 0. {
                    output[next] = a;
                    next += 1;
                }
                if (da >= 0.) != (db >= 0.) {
                    output[next] = add(a, mul(sub(b, a), da / (da - db)));
                    next += 1;
                }
            }
            if next < 3 {
                return;
            }
            input = output;
            count = next;
        }
        for i in 1..count - 1 {
            for p in [input[0], input[i], input[i + 1]] {
                out.push(Vertex {
                    position: self.project(p),
                    color: [rgb[0], rgb[1], rgb[2], 1.],
                });
            }
        }
    }
    fn line(&self, a: Vec3, b: Vec3, rgb: Vec3, width: f32, out: &mut Vec<Vertex>) {
        let a = self.camera_point(a);
        let b = self.camera_point(b);
        let (mut enter, mut leave) = (0f32, 1f32);
        for plane in 0..5 {
            let da = self.plane(a, plane);
            let db = self.plane(b, plane);
            if da < 0. && db < 0. {
                return;
            }
            if (da >= 0.) != (db >= 0.) {
                let t = da / (da - db);
                if da < 0. {
                    enter = enter.max(t);
                } else {
                    leave = leave.min(t);
                }
            }
        }
        if enter > leave {
            return;
        }
        let delta = sub(b, a);
        let b = add(a, mul(delta, leave));
        let a = add(a, mul(delta, enter));
        let a = self.project(a);
        let b = self.project(b);
        let dx = (b[0] / b[3] - a[0] / a[3]) * self.camera.aspect;
        let dy = b[1] / b[3] - a[1] / a[3];
        let length = (dx * dx + dy * dy).sqrt();
        if length < 1e-6 {
            return;
        }
        let x = -dy / length * width / self.camera.aspect;
        let y = dx / length * width;
        let corner = |p: [f32; 4], sign: f32| {
            [
                p[0] + x * p[3] * sign,
                p[1] + y * p[3] * sign,
                p[2] - 0.0002 * p[3],
                p[3],
            ]
        };
        let corners = [corner(a, -1.), corner(a, 1.), corner(b, 1.), corner(b, -1.)];
        for i in [0, 1, 2, 0, 2, 3] {
            out.push(Vertex {
                position: corners[i],
                color: [rgb[0], rgb[1], rgb[2], 1.],
            });
        }
    }
    fn mesh(
        &self,
        mesh: &Mesh,
        position: Vec3,
        scale: Vec3,
        rgb: Vec3,
        angle: f32,
        out: &mut Vec<Vertex>,
    ) {
        let rotation = Rotation::new(0., angle);
        for face in &mesh.faces {
            let n = rotation.apply(unit([
                face.normal[0] / scale[0],
                face.normal[1] / scale[1],
                face.normal[2] / scale[2],
            ]));
            let points = face.points.map(|p| {
                add(
                    position,
                    rotation.apply([p[0] * scale[0], p[1] * scale[1], p[2] * scale[2]]),
                )
            });
            let n = self.model.apply(n);
            if dot(n, sub(self.eye, self.model.apply(points[0]))) <= 0. {
                continue;
            }
            let light = 0.22 + 0.65 * dot(n, [-0.36, 0.48, 0.8]).max(0.);
            let shine = 0.45 * dot(n, [0., 0.2, 0.9798]).max(0.).powi(12);
            self.triangle(
                points,
                [
                    (rgb[0] * light + shine).min(1.),
                    (rgb[1] * light + shine * 0.8).min(1.),
                    (rgb[2] * light + shine * 0.4).min(1.),
                ],
                out,
            );
        }
    }
    fn limb(&self, a: Vec3, b: Vec3, radius: f32, rgb: Vec3, out: &mut Vec<Vertex>) {
        let axis = unit(sub(b, a));
        let u = unit(cross(axis, [0., 0., 1.]));
        let v = cross(axis, u);
        for i in 0..6 {
            let radial = |i: usize| {
                add(
                    mul(u, (TAU * i as f32 / 6.).cos()),
                    mul(v, (TAU * i as f32 / 6.).sin()),
                )
            };
            let r = mul(radial(i), radius);
            let s = mul(radial(i + 1), radius);
            let color = mul(
                rgb,
                0.28 + 0.65 * dot(self.model.apply(radial(i)), [-0.36, 0.48, 0.8]).max(0.),
            );
            self.triangle([add(a, r), add(b, r), add(b, s)], color, out);
            self.triangle([add(a, r), add(b, s), add(a, s)], color, out);
        }
    }
}
