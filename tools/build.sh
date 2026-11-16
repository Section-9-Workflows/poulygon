#!/bin/sh
set -eu
mkdir -p build/objects dist/bin
pb=vendor/poulsback
includes="-Isrc -I$pb/src/compiler -I$pb/src/raster -I$pb/include/uapi/poulsback -I$pb/src/gallium/mesa/poulsback"
flags="-std=c11 -O3 -march=bonnell -mtune=bonnell -flto=auto -ffunction-sections -fdata-sections -fno-math-errno -D_FILE_OFFSET_BITS=64 -DPOULSBACK_SELFCHECK=0 -Wall -Wextra"
objects=
while IFS= read -r source; do
    object="build/objects/$(basename "$source" .c).o"
    cc $flags $includes $(pkg-config --cflags libdrm) -c "$pb/$source" -o "$object"
    objects="$objects $object"
done < tools/native-sources.txt
cc $flags -Werror $includes $(pkg-config --cflags libdrm) -c src/native.c -o build/objects/native.o
gcc-ar rcs build/libfacet_native.a $objects build/objects/native.o
rustc --edition=2021 -D warnings -C opt-level=3 -C target-cpu=bonnell \
    -C lto=fat -C codegen-units=1 -C panic=abort -C strip=symbols \
    -L native=build -l static=facet_native -l drm -l m \
    -C link-arg=-flto=auto -C link-arg=-Wl,--gc-sections,-Map,build/link.map \
    src/main.rs -o dist/bin/facet
python3 tools/package.py
