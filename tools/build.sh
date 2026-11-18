#!/bin/sh
set -eu
mkdir -p dist/bin
sh tools/build-native.sh build
rustc --edition=2021 --crate-type=rlib --crate-name poulygon --cfg 'feature="native"' \
    -D warnings -C opt-level=3 -C target-cpu=bonnell src/lib.rs --out-dir build
rustc --edition=2021 -D warnings -C opt-level=3 -C target-cpu=bonnell \
    -C lto=fat -C codegen-units=1 -C panic=abort -C strip=symbols \
    -L native=build -l static=facet_native -l drm -l m \
    -C link-arg=-flto=auto -C link-arg=-Wl,--gc-sections,-Map,build/link.map \
    src/main.rs -o dist/bin/facet
python3 tools/package.py
