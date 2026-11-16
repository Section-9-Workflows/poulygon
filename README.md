# Facet

A small Rust application for flat-shaded 3D on the Sony VAIO P, using Poulsback
directly. The first scene is a rotating cube at the inherited panel resolution
(1600×768 on Icarus). It needs no X, Wayland, OpenGL or Mesa runtime.

Rust transforms eight corners, culls hidden faces and calculates diffuse face colors. The GPU
clips and rasterizes the triangles, handles depth, and writes a native scanout
buffer. Two buffers alternate through KMS page flips, paced by display events.
The frame writer caches programs and state. The draw loop uses fixed arrays.
No texture assets, runtime crate dependencies, per-pixel CPU shading or CPU copy
of the completed image is used.

The simulation uses numbered 120 Hz ticks independently of display timing.
This leaves a place for future timestamped inputs and snapshots. There is no
network transport, rollback system or claim of cross-machine determinism yet.

## Run

Extract the release into its own directory, then run `./facet` from a local
terminal. The launcher opens a spare Linux VT with sudo. Q, Escape or Ctrl-C
exits; switching VTs also exits. `facet.log` records startup, frame rate and
errors. The launcher waits for exit and returns to the original console.

The package carries a private i386 loader and shared libraries, leaving the
old host runtime intact. It does not replace fbterm or install a system service.
Close the demo before lid sleep; concurrent suspend has not been qualified.

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

Use the pinned i386 Debian container in `tools/Dockerfile` through Depot:

```
depot build --project qk39vv1js0 --build-platform linux/amd64 --platform linux/386 \
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
