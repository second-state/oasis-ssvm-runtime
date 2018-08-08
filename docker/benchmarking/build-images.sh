#!/bin/bash -e

ekiden_image=${EKIDEN_DOCKER_IMAGE:-ekiden/development:0.2.0}
base_dir=$( cd "$( dirname "${BASH_SOURCE[0]}" )/../.." && pwd )

cd ${base_dir}

if [ -n "$BUILD_IMAGES_NO_ENTER" ]; then
    ./docker/benchmarking/build-images-inner.sh
elif [ -z "$BUILD_IMAGES_CONTAINER" ]; then
    # Build in a fresh container.
    docker run --rm \
        -v "$PWD:/code" \
        -e SGX_MODE=SIM \
        -e INTEL_SGX_SDK=/opt/sgxsdk \
        -w /code \
        "$ekiden_image" \
        /code/docker/benchmarking/build-images-inner.sh
else
    # Build in a specified container.
    docker exec "$BUILD_IMAGES_CONTAINER" \
        /code/docker/benchmarking/build-images-inner.sh
fi

# Build the deployable image from the output.
docker build --rm --force-rm -t oasislabs/ekiden-runtime-ethereum:benchmarking-latest - <target/docker-benchmarking/context.tar.gz