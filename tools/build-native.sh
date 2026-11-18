#!/bin/sh
set -eu
output=${1:-build}
mkdir -p "$output/objects"
pb=vendor/poulsback
includes="-Isrc -I$pb/src/compiler -I$pb/src/raster -I$pb/include/uapi/poulsback -I$pb/src/gallium/mesa/poulsback"
flags="-std=c11 -O3 -march=bonnell -mtune=bonnell -flto=auto -ffunction-sections -fdata-sections -fno-math-errno -D_FILE_OFFSET_BITS=64 -DPOULSBACK_SELFCHECK=0 -Wall -Wextra ${CFLAGS:-}"
objects=
while IFS= read -r source; do
    object="$output/objects/$(basename "$source" .c).o"
    ${CC:-cc} $flags $includes $(${PKG_CONFIG:-pkg-config} --cflags libdrm) -c "$pb/$source" -o "$object"
    objects="$objects $object"
done < tools/native-sources.txt
${CC:-cc} $flags -Werror $includes $(${PKG_CONFIG:-pkg-config} --cflags libdrm) -c src/native.c -o "$output/objects/native.o"
${CC:-cc} $flags -Werror -c src/native_input.c -o "$output/objects/native_input.o"
${AR:-gcc-ar} rcs "$output/libfacet_native.a" $objects "$output/objects/native.o" "$output/objects/native_input.o"
