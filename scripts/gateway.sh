#!/bin/bash
# Usage:
#   RUNTIME_BUILD_DIR=/path/to/target/maybe-sgx-triple/(mode=debug|release) \
#   GATEWAY_BUILD_DIR=/path/to/target/default/mode \
#   GENESIS_ROOT_PATH=/path/to/resources/genesis \
#   gateway.sh

set -euo pipefail

sgxs=""
tee_hardware=""
if [[ "${RUNTIME_BUILD_DIR}" == *"sgx"* ]]; then
    sgxs=".sgxs"
    tee_hardware="--fixture.default.tee_hardware intel-sgx"
fi

oasis_node="${OASIS_CORE_ROOT_PATH}/go/oasis-node/oasis-node"
oasis_runner="${OASIS_CORE_ROOT_PATH}/go/oasis-net-runner/oasis-net-runner"
runtime_loader="${OASIS_CORE_ROOT_PATH}/target/default/debug/oasis-core-runtime-loader"
keymanager_binary="${RUNTIME_BUILD_DIR}/oasis-ssvm-runtime-keymanager${sgxs}"
runtime_binary="${RUNTIME_BUILD_DIR}/oasis-ssvm-runtime${sgxs}"
runtime_genesis="${GENESIS_ROOT_PATH}/oasis_genesis_testing.json"
web3_gateway="${GATEWAY_BUILD_DIR:-${RUNTIME_BUILD_DIR}}/gateway"

# Prepare an empty data directory.
data_dir="/var/tmp/oasis-ssvm-runtime-runner"
rm -rf "${data_dir}/net-runner/network"
mkdir -p "${data_dir}"
chmod -R go-rwx "${data_dir}"
client_socket="${data_dir}/net-runner/network/client-0/internal.sock"

# Dump
echo "Dump fixture."
${oasis_runner} dump-fixture \
    ${tee_hardware} \
    --fixture.default.node.binary ${oasis_node} \
    --fixture.default.runtime.binary ${runtime_binary} \
    --fixture.default.runtime.loader ${runtime_loader} \
    --fixture.default.runtime.genesis_state ${runtime_genesis} \
    --fixture.default.keymanager.binary ${keymanager_binary} \
    --fixture.default.epochtime_mock \
    > fixture.json
sed -i 's/"runtime_provisioner": ""/"runtime_provisioner": "unconfined"/' fixture.json

# Run the network.
echo "Starting the test network from fixture.json."
${oasis_runner} \
    --fixture.file fixture.json \
    --basedir.no_temp_dir \
    --basedir ${data_dir} &

# Wait for the validator and keymanager nodes to be registered.
echo "Waiting for the validator and keymanager to be registered."
${oasis_node} debug control wait-nodes \
    --address unix:${client_socket} \
    --nodes 2 \
    --wait

# Advance epoch.
echo "Advancing epoch."
${oasis_node} debug control set-epoch \
    --address unix:${client_socket} \
    --epoch 1

# Wait for all nodes to be registered.
echo "Waiting for all nodes to be registered."
${oasis_node} debug control wait-nodes \
    --address unix:${client_socket} \
    --nodes 6 \
    --wait

# Advance epoch.
echo "Advancing epoch."
${oasis_node} debug control set-epoch \
    --address unix:${client_socket} \
    --epoch 2

# Start the gateway.
echo "Starting the web3 gateway."
${web3_gateway} \
    --node-address unix:${client_socket} \
    --runtime-id 8000000000000000000000000000000000000000000000000000000000000000
