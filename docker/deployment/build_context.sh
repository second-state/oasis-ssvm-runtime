#!/bin/bash

# Build a Docker context tarball.

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

###############
# Required args
###############
dst="$1"

# Read oasis core version.
oasis_core_version="$(cat OASIS_CORE_VERSION)"

OASIS_UNSAFE_SKIP_AVR_VERIFY=1
export OASIS_UNSAFE_SKIP_AVR_VERIFY
OASIS_UNSAFE_KM_POLICY_KEYS=1
export OASIS_UNSAFE_KM_POLICY_KEYS

# Download oasis-core artifacts.
curl -L -o oasis_core_linux_amd64.tar.gz \
 "https://github.com/oasisprotocol/oasis-core/releases/download/v${oasis_core_version}/oasis_core_${oasis_core_version}_linux_amd64.tar.gz"
mkdir -p oasis-core
tar -C oasis-core -xzf oasis_core_linux_amd64.tar.gz
mv "oasis-core/oasis_core_${oasis_core_version}_linux_amd64/oasis-node" oasis-core/
mv "oasis-core/oasis_core_${oasis_core_version}_linux_amd64/oasis-core-runtime-loader" oasis-core/

# Install oasis-core-tools.
cargo install \
    --force \
    --git https://github.com/oasisprotocol/oasis-core \
    --tag "v$oasis_core_version" \
    oasis-core-tools


# Build the runtime.
cargo build --release ${RUNTIME_BUILD_EXTRA_ARGS:-}
cargo build --release --target x86_64-fortanix-unknown-sgx ${RUNTIME_BUILD_EXTRA_ARGS:-}
cargo elf2sgxs --release

# Build the keymanager-runtime.
pushd keymanager-runtime
    # Make sure UNSAFE_SKIP_KM_POLICY is set.
    OASIS_UNSAFE_SKIP_KM_POLICY=1 cargo build --release
    # Make sure UNSAFE_SKIP_KM_POLICY is unset.
    unset OASIS_UNSAFE_SKIP_KM_POLICY
    cargo build --release --target x86_64-fortanix-unknown-sgx
    cargo elf2sgxs --release
popd

# Build the gateway.
(
    cd gateway
    cargo build -p web3-gateway \
        --release \
        ${GATEWAY_BUILD_EXTRA_ARGS:-}
)

# Copy the correct genesis file.
if [ -n "${BUILD_PRODUCTION_GENESIS:-}" ]; then
    cp resources/genesis/oasis_genesis.json resources/genesis.json
else
    cp resources/genesis/oasis_genesis_testing.json resources/genesis.json
fi

tar -czf "$dst" \
    resources/genesis.json \
    target/release/oasis-ssvm-runtime \
    target/x86_64-fortanix-unknown-sgx/release/oasis-ssvm-runtime.sgxs \
    target/release/oasis-ssvm-runtime-keymanager \
    target/x86_64-fortanix-unknown-sgx/release/oasis-ssvm-runtime-keymanager.sgxs \
    target/release/gateway \
    oasis-core/ \
    OASIS_CORE_VERSION \
    docker/deployment/Dockerfile
