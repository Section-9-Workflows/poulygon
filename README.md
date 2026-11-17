# Poulygon

A compact Rust engine for flat-shaded 3D on the Sony VAIO P, using Poulsback
directly at the inherited panel resolution (1600×768 on Icarus). It needs no X,
Wayland, OpenGL or Mesa runtime. The original cube and an automatic workload
explorer share the same native display backend.

Rust transforms geometry and calculates diffuse face colors. The GPU clips and
rasterizes the triangles, handles depth and writes scanout buffers. Two buffers
alternate through KMS page flips. This version does not execute transforms or
lighting on the GPU. It uses no textures or runtime crate dependencies.

## Run

Extract the release into its own directory, then run `./poulygon` from a local
terminal. This starts the explorer. `./poulygon cube` selects the original cube.
The launcher opens a spare Linux VT with sudo. Q, Escape or Ctrl-C exits;
switching VTs also exits. The launcher returns to the original console.

Each launch creates a directory under `~/.local/state/poulygon/runs/` (or
`$XDG_STATE_HOME/poulygon/runs/`). It prints the path before switching consoles:

- `events.jsonl`: scene parameters, measurement windows, decisions and errors.
- `summary.md`: the latest steady-window result and passing levels by family.
- `native.log`: native backend errors and readable progress.

An idle inhibitor prevents automatic battery suspend while the application runs.
It does not override lid-close or manual suspend. Close the application before
lid sleep; concurrent suspend has not been qualified. The private i386 loader
and shared libraries leave the host runtime and fbterm installation intact.

## Automatic explorer

The explorer runs until you quit. There are no tuning controls. It increases one
workload at a time, backs down after a failure, then refines the interval between
the last passing and failing levels. Two passing ten-second windows are required
before increasing a level. Each scene change gets two seconds to settle.

A passing window needs at least 29.5 FPS and a presentation-interval p99 of at most
35 ms. A one-second interval below 27 FPS, or with more than 20% of frames above
35 ms, triggers an early reduction. Those partial intervals are logged separately.
Scene setup and settling do not contribute to steady-window statistics.

| Family | What changes |
|---|---|
| Panel corridor | Growing floor/wall panel and solid counts, with faster forward travel |
| Object field | More cubes, octahedra and low-poly spheres along a moving path |
| Polygon city | Larger solids beside a clear central path, with corridor panels |
| Geometry | Subdivisions of one rotating flat-shaded sphere |
| Coverage | Projected size, through 16 size levels |
| Overlap, front/back | Number of overlapping objects, in each submission order |
| CPU lights | Number of moving diffuse lights evaluated per visible face |
| Simulation | Active spring-driven objects, within a fixed 128-object scene |
| Combined | Object count, using geometry and lighting levels from prior observations |

The three fly-through scenes run first. Each level increases object counts and
travel speed. Speed ramps smoothly, and motion continues between fly-through
levels. The scene wraps ahead of the camera to sustain travel without building
an unbounded world. Later rounds also vary the speed and mixed scene. The run is repeatable from its recorded
parameters, but it is not a claim of cross-machine floating-point determinism.
Capacity admission uses the full unculled vertex count. It is deliberately
conservative: a scene must fit even when rotation exposes different faces.
Current storage allows 32,766 expanded vertices, batched into at most 4,095
vertices per draw. Capacity refusal is recorded separately from a missed frame
budget. It does not imply that the GPU has reached its performance limit.

Logs include actual submitted vertices, triangles and draw counts; frame interval
percentiles; CPU simulation, geometry/lighting, frame construction, submission,
completion wait and presentation wait; process CPU use and resident memory;
plane counts, target and actual travel speed; and available thermal, clock and
power readings. Completion wait includes driver
and scheduling time, so it is not an isolated GPU timing measurement. Missing
system readings are empty strings. Run metadata includes source and driver versions.

## Engine layout

- `engine.rs`: vectors, camera, meshes, rotations and a numbered 120 Hz simulation clock.
- `platform.rs` / `native.c`: narrow Rust/C boundary, GPU resources and VT ownership.
- `cube.rs` / `scene.rs`: application geometry and simulation.
- `explorer.rs`: workload selection and boundary refinement.
- `telemetry.rs`: measurement windows and run records.
- `main.rs`: application loop and lifecycle.

Simulation executes fixed steps independently of rendering. Scenes share the
camera, mesh and display code, and reuse vertex storage during rendering. Future
exploration, racing or music scenes can replace scene logic without duplicating
DRM setup. There is no editor, plugin system, network transport or physics framework.

## Native integration

`src/native.c` owns DRM buffers, completion fences, the display and the terminal.
Rust calls that narrow C interface. The native frame writer and command/compiler
libraries are pinned from Section-9-Workflows/poulsback at the revision recorded
in `vendor/poulsback/REVISION`, with original licenses and source hashes retained.
They are compiled without the Gallium frontend or Mesa NIR adapters. The frame
writer resides in upstream's Gallium directory but its interface is standalone.

Every submitted frame is drained before its command storage is reused. Fence
errors and uncertain completion stop the application without retrying or resetting
the GPU. The kernel retains resources still reachable by the device. A driver
fault can still require a reboot; this application cannot repair a lost engine.

## Build

Pushes to `main` and the manual GitHub Actions workflow build the pinned i386
container on Depot. GitHub coordinates the job; compilation runs on Depot's
AMD64 builder. The workflow uses OIDC, with no stored Depot credential. Public
fork pull requests cannot use this project's build credentials.

Depot keeps the build cache and the finished artifact image. The workflow
summary links the build and records its source revision. It does not upload
GitHub Actions artifacts. The image contains `poulygon-i686.tar.gz`, source,
compiler details and `SHA256SUMS`.

To build and export those files from a machine with an authenticated Depot CLI:

```
depot build --build-platform linux/amd64 --platform linux/386 \
  --file tools/Dockerfile --build-arg SOURCE_REVISION="$(git rev-parse HEAD)" \
  --save --output type=local,dest=artifacts .
```

Rust uses Bonnell tuning, optimization level 3, whole-program LTO, one codegen
unit and abort-on-panic. C uses Bonnell tuning, level 3, LTO and section elimination.
Floating-point correctness and the driver's ownership checks are preserved.
The artifact includes source, compiler/package versions and checksums.

Compilation does not establish GPU behavior. Native frame rate, visual output,
console restoration and power use need actual VAIO P validation. These settings
are a performance baseline, not proof of the fastest implementation.
