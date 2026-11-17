// SPDX-License-Identifier: MIT
use crate::engine::*;
use crate::platform::Vertex;

pub const FAMILIES: [&str; 10] = [
    "geometry",
    "coverage",
    "overlap_front",
    "overlap_back",
    "cpu_lights",
    "simulation",
    "combined",
    "panel_corridor",
    "object_field",
    "polygon_city",
];
#[derive(Clone, Copy)]
pub struct Config {
    pub family: usize,
    pub level: usize,
    pub cycle: u64,
    pub detail: usize,
    pub objects: usize,
    pub lights: usize,
    pub active: usize,
    pub scale: f32,
    pub planes: usize,
    pub speed: f32,
}
impl Config {
    pub fn new(family: usize, level: usize, cycle: u64, safe: &[usize; FAMILIES.len()]) -> Self {
        let mut c = Self {
            family,
            level,
            cycle,
            detail: 1,
            objects: 1,
            lights: 1,
            active: 0,
            scale: 1.0,
            planes: 0,
            speed: 0.,
        };
        match family {
            0 => c.detail = level,
            1 => {
                c.detail = 4;
                c.scale = 0.2 + level as f32 * 0.2;
            }
            2 | 3 => {
                c.objects = level;
                c.scale = 1.6;
            }
            4 => {
                c.detail = 6;
                c.lights = level;
            }
            5 => {
                c.objects = 128;
                c.active = level.min(128);
                c.scale = 0.17;
            }
            7..=9 => {
                c.objects = level.saturating_mul(if family == 8 { 24 } else { 8 });
                c.planes = if family == 8 {
                    0
                } else {
                    level.saturating_mul(32)
                };
                c.scale = if family == 9 { 0.5 } else { 0.3 };
                c.speed = 1.5 + (level as f32).log2() * 1.25 + (cycle % 3) as f32 * 0.5;
            }
            _ => {
                // Each round rotates which previously measured axis shapes the mixed scene.
                c.detail = (safe[0].max(1) / (2 + (cycle as usize % 3))).max(1).min(12);
                c.objects = level;
                c.active = level;
                c.scale = 0.35;
                c.lights = (safe[4].max(1) / (2 + ((cycle as usize + 1) % 3))).max(1);
            }
        }
        c
    }
    pub fn domain_max(family: usize) -> usize {
        match family {
            1 => 16,
            5 => 128,
            _ => usize::MAX / 4,
        }
    }
    pub fn json(&self) -> String {
        format!("\"family\":\"{}\",\"level\":{},\"cycle\":{},\"detail\":{},\"objects\":{},\"lights\":{},\"simulated\":{},\"scale\":{},\"planes\":{},\"target_speed\":{}",FAMILIES[self.family],self.level,self.cycle,self.detail,self.objects,self.lights,self.active,self.scale,self.planes,self.speed)
    }
}
struct Actor {
    home: Vec3,
    pos: Vec3,
    velocity: Vec3,
}
pub struct Scene {
    pub config: Config,
    mesh: Mesh,
    solids: [Mesh; 2],
    plane: Mesh,
    travel: f64,
    speed: f32,
    actors: Vec<Actor>,
    lights: Vec<Vec3>,
    time: f32,
    pub source_vertices: usize,
}
impl Scene {
    pub fn new(config: Config, capacity: usize) -> Result<Self, &'static str> {
        let rows = config.detail.checked_add(2).ok_or("mesh capacity")?;
        let triangles = rows
            .checked_mul(rows - 1)
            .and_then(|x| x.checked_mul(4))
            .ok_or("mesh capacity")?;
        let object_vertices = if config.family >= 7 {
            let group = triangles.checked_mul(3).and_then(|n| n.checked_add(60));
            group
                .and_then(|n| n.checked_mul(config.objects / 3))
                .and_then(|n| n.checked_add([0, 36, 60][config.objects % 3]))
        } else {
            triangles
                .checked_mul(3)
                .and_then(|n| n.checked_mul(config.objects))
        };
        let vertices = object_vertices
            .and_then(|n| config.planes.checked_mul(6).and_then(|p| n.checked_add(p)))
            .ok_or("mesh capacity")?;
        // Admission uses the full mesh, so camera motion cannot overflow a previously accepted scene.
        if vertices > capacity || config.lights > capacity {
            return Err("renderer capacity");
        }
        let mesh = Mesh::sphere(config.detail);
        let columns = (config.objects as f32).sqrt().ceil() as usize;
        let actors = (0..config.objects)
            .map(|i| {
                let home = if config.family >= 7 {
                    let x = ((i as f32 * 0.618034).fract() * 2. - 1.) * 5.;
                    [
                        if config.family == 9 {
                            x.signum() * (x.abs() + 0.9)
                        } else {
                            x
                        },
                        if config.family == 9 {
                            -1.2
                        } else {
                            ((i as f32 * 0.7548777).fract() * 2. - 1.) * 2.2
                        },
                        i as f32 / config.objects as f32 * 16.,
                    ]
                } else if config.family == 2 || config.family == 3 {
                    [0., 0., -(i as f32) * 0.015]
                } else if config.objects == 1 {
                    [0., 0., 0.]
                } else {
                    [
                        (i % columns) as f32 / columns as f32 * 7.0 - 3.5,
                        (i / columns) as f32 / columns as f32 * 3.2 - 1.6,
                        -0.2,
                    ]
                };
                Actor {
                    home,
                    pos: home,
                    velocity: [0.; 3],
                }
            })
            .collect();
        Ok(Self {
            config,
            mesh,
            solids: [Mesh::solid(true), Mesh::solid(false)],
            plane: Mesh::plane(),
            travel: 0.,
            speed: 0.,
            actors,
            lights: vec![[0.; 3]; config.lights],
            time: 0.,
            source_vertices: vertices,
        })
    }
    pub fn inherit_motion(&mut self, previous: &Self) {
        if self.config.family >= 7 && previous.config.family >= 7 {
            self.time = previous.time;
            self.travel = previous.travel;
            self.speed = previous.speed;
        }
    }
    pub fn speed(&self) -> f32 {
        self.speed
    }
    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        self.speed += (self.config.speed - self.speed) * (dt * 0.8);
        self.travel = (self.travel + dt as f64 * self.speed as f64).rem_euclid(16.);
        for i in 0..self.config.active {
            let neighbor = if i == 0 {
                self.actors[i].home
            } else {
                self.actors[i - 1].pos
            };
            let a = &mut self.actors[i];
            let wave = [
                (self.time * 1.3 + i as f32).sin() * 0.18,
                (self.time + i as f32 * 0.7).cos() * 0.12,
                0.,
            ];
            let force = add(
                mul(sub(add(a.home, wave), a.pos), 7.0),
                mul(sub(neighbor, a.pos), 0.15),
            );
            a.velocity = add(a.velocity, mul(sub(force, mul(a.velocity, 1.5)), dt));
            a.pos = add(a.pos, mul(a.velocity, dt));
        }
    }
    pub fn draw(&mut self, camera: &Camera, out: &mut Vec<Vertex>) {
        out.clear();
        for (i, light) in self.lights.iter_mut().enumerate() {
            let angle = self.time * 0.4 + i as f32 * 2.399963;
            *light = [angle.cos() * 0.6, angle.sin() * 0.6, 0.8];
        }
        for ordinal in 0..self.actors.len() {
            let i = if self.config.family == 3 {
                self.actors.len() - 1 - ordinal
            } else {
                ordinal
            };
            let actor = &self.actors[i];
            let rotation = Rotation::new(self.time * 0.31 + i as f32 * 0.2, self.time * 0.55);
            let hue = i as f32 * 2.399963 + self.config.family as f32;
            let rgb = [
                0.55 + 0.4 * hue.sin(),
                0.55 + 0.4 * (hue + 2.1).sin(),
                0.55 + 0.4 * (hue + 4.2).sin(),
            ];
            let mut position = actor.pos;
            if self.config.family >= 7 {
                position[0] -= (self.time * 0.23).sin() * 0.5;
                position[1] -= (self.time * 0.31).sin() * 0.15;
                position[2] = 2. - (actor.home[2] as f64 - self.travel).rem_euclid(16.) as f32;
            }
            let mesh = if self.config.family >= 7 && i % 3 != 2 {
                &self.solids[i % 3]
            } else {
                &self.mesh
            };
            emit(
                mesh,
                &rotation,
                position,
                self.config.scale,
                rgb,
                &self.lights,
                camera,
                out,
                false,
            );
        }
        for i in 0..self.config.planes {
            let side = i % 4;
            let z = 2.
                - (i as f64 / self.config.planes as f64 * 16. - self.travel).rem_euclid(16.) as f32;
            let (mut position, rotation) = match side {
                0 => (
                    [0., -1.8, z],
                    Rotation::new(std::f32::consts::FRAC_PI_2, 0.),
                ),
                1 => ([0., 1.8, z], Rotation::new(std::f32::consts::FRAC_PI_2, 0.)),
                2 => (
                    [-3.2, 0., z],
                    Rotation::new(0., std::f32::consts::FRAC_PI_2),
                ),
                _ => ([3.2, 0., z], Rotation::new(0., std::f32::consts::FRAC_PI_2)),
            };
            position[0] -= (self.time * 0.23).sin() * 0.5;
            position[1] -= (self.time * 0.31).sin() * 0.15;
            let rgb = if (i / 4) % 2 == 0 {
                [0.15, 0.7, 0.95]
            } else {
                [0.95, 0.3, 0.55]
            };
            emit(
                &self.plane,
                &rotation,
                position,
                0.75,
                rgb,
                &self.lights,
                camera,
                out,
                true,
            );
        }
    }
}

fn emit(
    mesh: &Mesh,
    rotation: &Rotation,
    position: Vec3,
    scale: f32,
    rgb: Vec3,
    lights: &[Vec3],
    camera: &Camera,
    out: &mut Vec<Vertex>,
    two_sided: bool,
) {
    for face in &mesh.faces {
        let n = rotation.apply(face.normal);
        let first = add(mul(rotation.apply(face.points[0]), scale), position);
        if !two_sided && dot(n, sub([0., 0., 5.], first)) <= 0. {
            continue;
        }
        let mut illumination = 0.;
        for light in lights {
            illumination += if two_sided {
                dot(n, *light).abs()
            } else {
                dot(n, *light).max(0.)
            };
        }
        let illumination = 0.22 + 0.78 * illumination / lights.len() as f32;
        let color = [
            rgb[0] * illumination,
            rgb[1] * illumination,
            rgb[2] * illumination,
            1.,
        ];
        for p in face.points {
            out.push(Vertex {
                position: camera.project(add(mul(rotation.apply(p), scale), position)),
                color,
            });
        }
    }
}
