# Facet / Icarus native 3D

No unit tests, synthetic probes, mocks, fabricated inputs or simulated devices.
Validate by compilation and the actual application on Icarus. The rotating cube
is the owner's requested application, not a test harness. Do not run inherited
Poulsback test programs or add a software renderer to substitute for hardware.

Use Depot for the i386 build and Nix for host tools. Never upgrade Icarus's OS
packages. Keep the owner’s fbterm sessions and tty7 fallback intact. Run on a
free Linux VT. Never retry, reset or unload after an unknown GPU outcome.

Keep the pinned upstream libraries unchanged and preserve their notices. Use
DoubleDensity <doubledense@gmail.com> for commits. No DCO certification by agents.
The application has no network code yet. Keep simulation independent of rendering;
add a real transport only with an actual multiplayer feature.
