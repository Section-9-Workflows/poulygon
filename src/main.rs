// SPDX-License-Identifier: MIT
// Copyright 2026 DoubleDensity <doubledense@gmail.com>
mod character;
mod cube;
mod cybersphere;
mod engine;
mod explorer;
mod platform;
mod scene;
mod telemetry;

use engine::{Camera, Clock};
use explorer::Explorer;
use platform::{Display, Vertex};
use scene::Scene;
use std::{path::PathBuf, time::Instant};
use telemetry::{quote, Log, Sample, Window};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn next_scene(explorer: &mut Explorer, capacity: usize, log: &mut Log) -> Result<Scene> {
    loop {
        let config = explorer.config();
        match Scene::new(config, capacity) {
            Ok(scene) => {
                log.event(
                    "scene",
                    &format!(
                        "{},\"source_vertices\":{}",
                        config.json(),
                        scene.source_vertices
                    ),
                )?;
                return Ok(scene);
            }
            Err(reason) => {
                log.event(
                    "capacity_refusal",
                    &format!("{},\"reason\":{}", config.json(), quote(reason)),
                )?;
                if config.level == 1 {
                    return Err("baseline scene exceeds renderer capacity".into());
                }
                explorer.result(false);
            }
        }
    }
}

fn play(display: &mut Display, log: &mut Log, app: &str) -> Result<()> {
    let camera = Camera {
        aspect: display.width as f32 / display.height as f32,
    };
    let mut clock = Clock::new();
    let mut explorer = Explorer::new();
    if app != "explore" {
        explorer.family = 0;
    }
    let mut scene = if app == "explore" {
        next_scene(&mut explorer, display.capacity(), log)?
    } else {
        Scene::new(explorer.config(), display.capacity())?
    };
    let sphere = if app == "cybersphere" {
        Some(cybersphere::Cybersphere::new())
    } else {
        None
    };
    let mut vertices = Vec::with_capacity(display.capacity());
    let mut cube_vertices = [Vertex::default(); 36];
    let mut window = Window::new();
    let mut previous = Instant::now();
    let mut presented = previous;
    let mut settled = previous;
    let (mut rolling_seconds, mut rolling_frames, mut rolling_missed) = (0.0, 0u32, 0u32);
    let mut frames = 0u64;
    loop {
        let now = Instant::now();
        clock.advance(now.duration_since(previous).as_secs_f64(), |dt| {
            if app == "explore" {
                scene.update(dt);
            }
        });
        previous = now;
        let simulation = now.elapsed().as_secs_f64() * 1000.0;
        let geometry_start = Instant::now();
        let span = if app == "cube" {
            let count = cube::draw(clock.time(), camera.aspect, &mut cube_vertices);
            &cube_vertices[..count]
        } else if let Some(sphere) = &sphere {
            sphere.draw(clock.time(), &camera, &mut vertices);
            &vertices[..]
        } else {
            scene.draw(&camera, &mut vertices);
            &vertices[..]
        };
        let geometry = geometry_start.elapsed().as_secs_f64() * 1000.0;
        let (result, native) = display.frame(span)?;
        if result == 0 {
            break;
        }
        if result == 2 {
            log.event(
                "capacity_refusal",
                &format!(
                    "{},\"reason\":\"native frame construction\"",
                    scene.config.json()
                ),
            )?;
            if app != "explore" || scene.config.level == 1 {
                return Err("baseline frame refused".into());
            }
            explorer.result(false);
        } else {
            frames += 1;
            let end = Instant::now();
            let interval = end.duration_since(presented).as_secs_f64() * 1000.0;
            presented = end;
            if settled.elapsed().as_secs_f64() < 2.0 {
                continue;
            }
            window.push(Sample {
                interval,
                simulation,
                geometry,
                native,
                vertices: span.len(),
            });
            rolling_seconds += interval / 1000.0;
            rolling_frames += 1;
            rolling_missed += u32::from(interval > 35.0);
            let slow = rolling_seconds >= 1.0
                && (rolling_frames as f64 / rolling_seconds < 27.0
                    || rolling_missed * 5 > rolling_frames);
            if slow && app == "explore" {
                log.event(
                    "sustained_slowdown",
                    &format!(
                        "{},\"fps\":{},\"seconds\":{},\"frames\":{},\"over_35ms\":{}",
                        scene.config.json(),
                        rolling_frames as f64 / rolling_seconds,
                        rolling_seconds,
                        rolling_frames,
                        rolling_missed
                    ),
                )?;
                explorer.result(false);
            } else {
                if rolling_seconds >= 1.0 {
                    rolling_seconds = 0.0;
                    rolling_frames = 0;
                    rolling_missed = 0;
                }
                if !window.ready() {
                    continue;
                }
                let (pass, mut fields, mut text) = window.report(&scene.config);
                if app != "explore" {
                    fields = fields.replacen(
                        &scene.config.json(),
                        &format!("\"family\":{}", quote(app)),
                        1,
                    );
                    text = text.replace("geometry at level 1", app);
                }
                fields.push_str(&format!(",\"actual_speed\":{}", scene.speed()));
                log.event("window", &fields)?;
                eprintln!("{}", text.trim());
                let transition = if app == "explore" {
                    explorer.result(pass)
                } else {
                    None
                };
                log.summary(&if app != "explore" {
                    text.clone()
                } else {
                    explorer.summary(&text)
                })?;
                window.clear();
                if let Some(reason) = transition {
                    log.event(
                        "transition",
                        &format!("\"reason\":{},{}", quote(reason), explorer.config().json()),
                    )?;
                } else {
                    continue;
                }
            }
        }
        let mut next = next_scene(&mut explorer, display.capacity(), log)?;
        next.inherit_motion(&scene);
        scene = next;
        window.clear();
        rolling_seconds = 0.0;
        rolling_frames = 0;
        rolling_missed = 0;
        clock = Clock::new();
        previous = Instant::now();
        presented = previous;
        settled = previous;
    }
    log.event(
        "stopped",
        &format!("\"frames\":{},\"simulation_tick\":{}", frames, clock.tick),
    )?;
    Ok(())
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let app = args.next().unwrap_or_else(|| "explore".into());
    if app != "cube" && app != "explore" && app != "cybersphere" {
        return Err("choose cube, explore or cybersphere".into());
    }
    if args.next().as_deref() != Some("--run-dir") {
        return Err("launch with the packaged poulygon script".into());
    }
    let directory = PathBuf::from(args.next().ok_or("missing run directory")?);
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let mut log = Log::new(directory)?;
    let mut display = Display::open()?;
    let result = (|| {
        log.start(display.width, display.height, display.capacity(), &app)?;
        play(&mut display, &mut log, &app)
    })();
    let cleanup = display.close();
    log.event(
        "exit",
        &format!(
            "\"playback_error\":{},\"cleanup_error\":{}",
            quote(
                &result
                    .as_ref()
                    .err()
                    .map(ToString::to_string)
                    .unwrap_or_default()
            ),
            quote(&cleanup.as_ref().err().cloned().unwrap_or_default())
        ),
    )?;
    result?;
    cleanup?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("poulygon: {}", error);
        std::process::exit(1);
    }
}
