// SPDX-License-Identifier: MIT
use crate::{character, engine::*, platform::Vertex};
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
    head: Mesh,
    torso: Mesh,
    time: f64,
    camera_angle: f32,
    camera_distance: f32,
    camera_y: f32,
    look_y: f32,
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
        for _ in 0..1 {
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
        let mut scene = Self {
            cells,
            targets,
            body: Mesh::sphere(1),
            head: character::head(),
            torso: character::torso(),
            time: 0.,
            camera_angle: 0.,
            camera_distance: 1.05,
            camera_y: 1.19,
            look_y: 1.15,
        };
        scene.camera_angle = scene.interaction(0.).3;
        scene
    }
    fn interaction(&self, time: f32) -> (usize, f32, f32, f32, f32, f32) {
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
        (active, previous_yaw, delta, yaw, phase, reach)
    }
    fn portrait(time: f32) -> f32 {
        let shot = (time + 30.).rem_euclid(40.);
        if (24.0..36.0).contains(&shot) {
            ((shot - 24.) / 12. * PI).sin().powi(2)
        } else {
            0.
        }
    }
    pub fn update(&mut self, dt: f32) {
        self.time += dt as f64;
        let time = self.time as f32;
        let portrait = Self::portrait(time);
        if portrait > 0.02 {
            let yaw = self.interaction(time).3;
            let turn = (yaw - self.camera_angle + PI).rem_euclid(TAU) - PI;
            self.camera_angle += (turn * dt * 1.4).clamp(-dt * 0.7, dt * 0.7);
        } else {
            self.camera_angle += dt * 0.18;
        }
        self.camera_angle = (self.camera_angle + PI).rem_euclid(TAU) - PI;
        self.camera_distance += (2.15 - 1.10 * portrait - self.camera_distance) * dt * 1.2;
        self.camera_y += (0.24 + 0.95 * portrait - self.camera_y) * dt * 1.2;
        self.look_y += (0.05 + 1.10 * portrait - self.look_y) * dt * 1.2;
    }
    pub fn draw(&self, time: f32, camera: &Camera, out: &mut Vec<Vertex>) {
        out.clear();
        let angle = self.camera_angle;
        let eye = [
            angle.sin() * self.camera_distance,
            self.camera_y,
            angle.cos() * self.camera_distance,
        ];
        let forward = unit(sub([0., self.look_y, 0.], eye));
        let right = unit(cross(forward, [0., 1., 0.]));
        let view = View {
            camera,
            eye,
            forward,
            right,
            up: cross(right, forward),
            model: Rotation::new(0., 0.),
            pivot: [0.; 3],
            offset: [0.; 3],
        };
        let (active, previous_yaw, delta, yaw, phase, reach) = self.interaction(time);
        let ease = (phase * 3.5).min(1.);
        let ease = ease * ease * (3. - 2. * ease);
        let target = self.cells[active].center;
        let portrait = Self::portrait(time);
        let toward_camera = (self.camera_angle - yaw + PI).rem_euclid(TAU) - PI;
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
        let world = view;
        let breathe = (time * 1.5).sin();
        let weight = (phase * TAU).sin();
        let offset = [weight * 0.035, -0.015 + 0.015 * breathe, 0.];
        let lean = 0.035 + reach * 0.06;
        let view = View {
            model: Rotation::new(
                lean,
                yaw + 0.08 * reach + toward_camera.clamp(-0.6, 0.6) * portrait * 0.65,
            ),
            pivot: [0., -0.35, 0.],
            offset,
            ..world
        };
        let hips = View {
            model: Rotation::new(0., yaw),
            offset,
            ..world
        };
        let face = View {
            model: Rotation::new(
                lean + 0.035 * (time * 0.8).sin(),
                yaw + (delta * (1. - ease)).clamp(-0.45, 0.45) * (1. - portrait)
                    + toward_camera.clamp(-0.6, 0.6) * portrait,
            ),
            pivot: [0., 0.9, 0.],
            offset,
            ..world
        };
        let gold = [0.91, 0.48, 0.1];
        let head = [0., 1.18, 0.];
        view.mesh(
            &self.torso,
            [0.; 3],
            [1. + breathe * 0.014, 1., 1. + breathe * 0.035],
            gold,
            0.,
            out,
        );
        hips.mesh(
            &self.body,
            [0., -0.35, 0.],
            [0.3, 0.23, 0.21],
            gold,
            0.,
            out,
        );
        view.limb([0., 0.83, 0.], [0., 1.02, 0.], 0.115, gold, out);
        face.mesh(&self.head, head, [1.; 3], gold, 0., out);
        let blink_phase = (time + 1.8).rem_euclid(4.6);
        let eyelid = if blink_phase < 0.20 {
            (blink_phase / 0.10 - 1.).abs().max(0.15)
        } else {
            1.
        };
        for side in [-1., 1.] {
            let shoulder = [side * 0.43, 0.65 + side * weight * 0.025, 0.];
            let hand_target = if side > 0. {
                [0.3, 0.57 + reach * 0.32, 0.35 + reach * 0.67]
            } else {
                [-0.75, 0.03 + 0.12 * (time * 0.7).sin(), 0.3 + reach * 0.12]
            };
            let (elbow, hand) =
                character::bend_joint(shoulder, hand_target, 0.56, 0.55, [side, 0., 0.]);
            view.limb(shoulder, elbow, 0.135, gold, out);
            view.limb(elbow, hand, 0.10, gold, out);
            view.mesh(&self.body, shoulder, [0.15; 3], gold, 0., out);
            view.mesh(&self.body, elbow, [0.10; 3], gold, 0., out);
            view.mesh(&self.body, hand, [0.105, 0.135, 0.065], gold, 0., out);
            if side > 0. && reach > 0.45 {
                world.line(
                    view.world_point(hand),
                    mul(target, 0.98),
                    [0.05, reach * 0.7, reach],
                    0.002,
                    out,
                );
            }
            for finger in 0..4 {
                let length = [0.105, 0.13, 0.12, 0.09][finger];
                let base = add(hand, [side * (finger as f32 - 1.5) * 0.041, 0.105, 0.]);
                let curl =
                    0.035 + 0.045 * (time * 0.85 + finger as f32 * 0.45).sin() * (1. - reach * 0.6);
                let first = add(
                    base,
                    [
                        side * (finger as f32 - 1.5) * 0.008,
                        length * 0.5,
                        curl * 0.35,
                    ],
                );
                let second = add(first, [0., length * 0.32, curl * 0.6]);
                let tip = add(second, [0., length * 0.18, curl * 0.5]);
                view.limb(base, first, 0.018, gold, out);
                view.limb(first, second, 0.015, gold, out);
                view.limb(second, tip, 0.012, gold, out);
            }
            view.limb(
                add(hand, [-side * 0.075, 0.015, 0.]),
                add(hand, [-side * 0.15, 0.09, 0.035]),
                0.025,
                gold,
                out,
            );
            let progress = ((phase - if side < 0. { 0. } else { 0.24 }) / 0.24).clamp(0., 1.);
            let smooth = progress * progress * (3. - 2. * progress);
            let foot_yaw = previous_yaw + delta * smooth;
            let lift = (progress * PI).sin().max(0.);
            let foot_rotation = Rotation::new(0., foot_yaw);
            let foot_target = add(
                foot_rotation.apply([side * 0.3, -1.60, 0.14]),
                [0., 0.23 * lift, 0.],
            );
            let hip = hips.world_point([side * 0.16, -0.43, 0.]);
            let (knee, foot) = character::bend_joint(
                hip,
                foot_target,
                0.58,
                0.61,
                foot_rotation.apply([0., 0., 1.]),
            );
            world.limb(hip, knee, 0.155, gold, out);
            world.limb(knee, foot, 0.105, gold, out);
            world.mesh(&self.body, knee, [0.10; 3], gold, 0., out);
            let foot_view = View {
                model: Rotation::new(-0.3 * lift, foot_yaw),
                pivot: foot,
                ..world
            };
            foot_view.mesh(&self.body, foot, [0.105, 0.075, 0.22], gold, 0., out);
            let eye = add(head, [side * 0.095, 0.07, 0.19]);
            face.mesh(
                &self.body,
                eye,
                [0.06, 0.023 * eyelid, 0.026],
                [0.12, 0.75, 0.95],
                0.,
                out,
            );
            face.mesh(
                &self.body,
                add(
                    eye,
                    [0.012 * (delta * (1. - ease)).clamp(-1., 1.), 0., 0.025],
                ),
                [0.019, 0.019 * eyelid, 0.008],
                [0.65, 1., 1.],
                0.,
                out,
            );
            face.mesh(
                &self.body,
                add(eye, [0., 0.055 + reach * 0.012, 0.]),
                [0.075, 0.016, 0.025],
                gold,
                0.,
                out,
            );
            face.mesh(
                &self.body,
                add(head, [side * 0.23, 0.015, -0.015]),
                [0.035, 0.085, 0.045],
                gold,
                0.,
                out,
            );
            let circuit = [
                [side * 0.11, 0.78, 0.18],
                [side * 0.28, 0.58, 0.23],
                [side * 0.23, 0.23, 0.22],
                [side * 0.12, 0.08, 0.2],
                [side * 0.12, -0.18, 0.16],
            ];
            for pair in circuit.windows(2) {
                view.line(pair[0], pair[1], [1., 0.72, 0.24], 0.0025, out);
            }
            face.line(
                add(head, [side * 0.17, 0.23, 0.13]),
                add(head, [side * 0.14, 0.13, 0.18]),
                [1., 0.65, 0.14],
                0.0018,
                out,
            );
            face.line(
                add(head, [side * 0.16, -0.03, 0.15]),
                add(head, [side * 0.12, -0.19, 0.17]),
                [1., 0.65, 0.14],
                0.0018,
                out,
            );
        }
        let mouth = add(head, [0., -0.16, 0.19]);
        let opening = 0.008 + 0.016 * reach * (0.5 + 0.5 * (time * 4.3).sin());
        face.mesh(
            &self.body,
            mouth,
            [0.075, opening, 0.022],
            [0.06, 0.018, 0.01],
            0.,
            out,
        );
        face.mesh(
            &self.body,
            add(mouth, [0., opening + 0.009, 0.005]),
            [0.082, 0.011, 0.024],
            [0.8, 0.30, 0.07],
            0.,
            out,
        );
        face.mesh(
            &self.body,
            add(mouth, [0., -opening - 0.009, 0.005]),
            [0.07, 0.013, 0.024],
            gold,
            0.,
            out,
        );
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
    pivot: Vec3,
    offset: Vec3,
}
const NEAR: f32 = 0.18;
impl View<'_> {
    fn world_point(&self, p: Vec3) -> Vec3 {
        add(
            add(self.model.apply(sub(p, self.pivot)), self.pivot),
            self.offset,
        )
    }
    fn camera_point(&self, p: Vec3) -> Vec3 {
        let v = sub(self.world_point(p), self.eye);
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
            if dot(n, sub(self.eye, self.world_point(points[0]))) <= 0. {
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
        let u = unit(cross(
            axis,
            if axis[2].abs() < 0.9 {
                [0., 0., 1.]
            } else {
                [0., 1., 0.]
            },
        ));
        let v = cross(axis, u);
        let profile: &[(f32, f32)] = if radius > 0.06 {
            &[(0., 0.72), (0.32, 1.), (0.68, 0.85), (1., 0.55)]
        } else {
            &[(0., 1.), (1., 0.8)]
        };
        let sides = if radius > 0.06 { 8 } else { 5 };
        for rings in profile.windows(2) {
            let point = |t: f32, r: f32, i: usize| {
                add(
                    add(a, mul(sub(b, a), t)),
                    mul(
                        add(
                            mul(u, (TAU * i as f32 / sides as f32).cos()),
                            mul(v, (TAU * i as f32 / sides as f32).sin()),
                        ),
                        radius * r,
                    ),
                )
            };
            for i in 0..sides {
                let p = point(rings[0].0, rings[0].1, i);
                let q = point(rings[0].0, rings[0].1, i + 1);
                let r = point(rings[1].0, rings[1].1, i);
                let s = point(rings[1].0, rings[1].1, i + 1);
                let n = self.model.apply(normal(p, q, r));
                if dot(n, sub(self.eye, self.world_point(p))) <= 0. {
                    continue;
                }
                let light = 0.25 + 0.65 * dot(n, [-0.36, 0.48, 0.8]).max(0.);
                self.triangle([p, q, r], mul(rgb, light), out);
                self.triangle([q, s, r], mul(rgb, light), out);
            }
        }
    }
}
