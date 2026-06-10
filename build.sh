#!/bin/sh
set -e
docker build -f Dockerfile.build -t bay-desktop-builder .
docker run --rm \
    -v "$PWD:/src" \
    -v bay-desktop-cargo:/root/.cargo/registry \
    -e HOST_UID=$(id -u) -e HOST_GID=$(id -g) \
    --cap-add SYS_ADMIN --device /dev/fuse \
    bay-desktop-builder --skip-tests --skip-lints --variant full "$@"
