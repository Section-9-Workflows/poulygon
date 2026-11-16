FROM docker.io/library/debian@sha256:139eacf457e4a322903c5ac9d5ac33da2b12bd1340e830124694a467a415a0d9 AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential rustc pkg-config libdrm-dev python3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /source
COPY . /source/
ARG SOURCE_REVISION
RUN --network=none setarch i686 sh tools/build.sh
RUN mkdir /artifacts && printf '%s\n' "$SOURCE_REVISION" > dist/source-revision.txt \
    && dpkg-query -W > /artifacts/build-packages.tsv \
    && rustc -vV > /artifacts/rustc.txt && cc -v 2> /artifacts/cc.txt \
    && readelf -h -d dist/bin/facet > /artifacts/elf.txt \
    && du -b dist/bin/facet > /artifacts/binary-size.txt \
    && tar -czf /artifacts/facet-i686.tar.gz -C dist . \
    && tar -czf /artifacts/source.tar.gz --exclude=.git --exclude=build --exclude=dist . \
    && cd /artifacts && sha256sum * > SHA256SUMS
FROM scratch AS artifacts
ARG SOURCE_REVISION
LABEL org.opencontainers.image.revision=$SOURCE_REVISION
COPY --from=build /artifacts/ /
