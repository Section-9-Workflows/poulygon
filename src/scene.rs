// SPDX-License-Identifier: MIT
use crate::engine::*;
use crate::platform::Vertex;

pub const FAMILIES: [&str; 7] = [
    "geometry",
    "coverage",
    "overlap_front",
    "overlap_back",
    "cpu_lights",
    "simulation",
    "combined",
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
}
impl Config {
    pub fn new(family: usize, level: usize, cycle: u64, safe: &[usize; 7]) -> Self {
        let mut c = Self {
            family,
            level,
            cycle,
            detail: 1,
            objects: 1,
            lights: 1,
            active: 0,
            scale: 1.0,
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
        format!("\"family\":\"{}\",\"level\":{},\"cycle\":{},\"detail\":{},\"objects\":{},\"lights\":{},\"simulated\":{},\"scale\":{}",FAMILIES[self.family],self.level,self.cycle,self.detail,self.objects,self.lights,self.active,self.scale)
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
        let vertices = triangles
            .checked_mul(3)
            .and_then(|n| n.checked_mul(config.objects))
            .ok_or("mesh capacity")?;
        // Admission uses the full mesh, so camera motion cannot overflow a previously accepted scene.
        if vertices > capacity || config.lights > capacity {
            return Err("renderer capacity");
        }
        let mesh = Mesh::sphere(config.detail);
        let columns = (config.objects as f32).sqrt().ceil() as usize;
        let actors = (0..config.objects)
            .map(|i| {
                let home = if config.family == 2 || config.family == 3 {
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
            actors,
            lights: vec![[0.; 3]; config.lights],
            time: 0.,
            source_vertices: vertices,
        })
    }
    pub fn update(&mut self, dt: f32) {
        self.time += dt;
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
            for face in &self.mesh.faces {
                let n = rotation.apply(face.normal);
                let first = add(
                    mul(rotation.apply(face.points[0]), self.config.scale),
                    actor.pos,
                );
                if dot(n, sub([0., 0., 5.], first)) <= 0. {
                    continue;
                }
                let mut illumination = 0.;
                for light in &self.lights {
                    illumination += dot(n, *light).max(0.);
                }
                let illumination = 0.22 + 0.78 * illumination / self.lights.len() as f32;
                let color = [
                    rgb[0] * illumination,
                    rgb[1] * illumination,
                    rgb[2] * illumination,
                    1.,
                ];
                for p in face.points {
                    out.push(Vertex {
                        position: camera
                            .project(add(mul(rotation.apply(p), self.config.scale), actor.pos)),
                        color,
                    });
                }
            }
        }
    }
}
