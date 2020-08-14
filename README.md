# Oasis SSVM/EVMC-VM Runtime

## Introduction

This project is provide a way follow Oasis [Eth/WASI Runtime](https://github.com/oasislabs/oasis-ethwasi-runtime) archtecture use SSVM/EVMC-VM.

## Getting Started

To get started with our demonstration, you will need to prepare three components at first.

- Pre-install Docker and pull our [docker image](https://hub.docker.com/r/secondstate/oasis-ssvm)
- Compatible Oasis-Core version.
- Our runtime
> The Cargo.toml inside this runtime will use our specific oasis-parity that embeded our ssvm.

## Preparation

- Pull official docker image to get an already established build environment.

```bash
docker pull secondstate/oasis-ssvm
```

- Get source code from Github.

```bash
git clone https://github.com/oasisprotocol/oasis-core.git --branch v20.9
git clone https://github.com/second-state/oasis-ssvm-runtime.git --branch ssvm
```

## Launch Environment
Attach shell to container, bind volume with repositories' path and specific in non-SGX environment.

```bash
docker run -it --rm \
  --name oasis-ssvm \
  --security-opt apparmor:unconfined \
  --security-opt seccomp=unconfined \
  -e OASIS_UNSAFE_SKIP_AVR_VERIFY=1 \
  -e OASIS_UNSAFE_SKIP_KM_POLICY=1 \
  -v $(pwd):/root/code \
  -w /root/code \
  secondstate/oasis-ssvm \
  bash
```

## Build runtime from source and running the network (in docker)

```bash
cd ~/code/oasis-ssvm-runtime
rustup target add x86_64-fortanix-unknown-sgx
make -C ../oasis-core
make symlink-artifacts OASIS_CORE_SRC_PATH=../oasis-core
make
make run-gateway
```

(wati for running gateway finish, maybe need more than 30 seconds)

The result should be the same as the following content.

```bash
 INFO  gateway/main > Starting the web3 gateway
 INFO  gateway/execute > Waiting for the Oasis Core node to be fully synced
 INFO  gateway/execute > Oasis Core node is fully synced, proceeding with initialization
 INFO  gateway/main    > Web3 gateway is running
```

## Deploy ERC20 (ewasm contract) and interact with it.

**Here we use the following ERC20 contract files as an example:**

- [erc20.sol](./resources/erc20/erc20.sol)
    - This file is an ERC20 contract written in Solidity.
- [erc20.wasm](./resources/erc20/erc20.wasm)
    - This file is a wasm file generate from `erc20.sol` by [SOLL](https://github.com/second-state/soll)
    - Command to generate wasm file: `soll -deploy=Normal erc20.sol`
- [erc20.hex](./resources/erc20/erc20.hex)
    - To deploy wasm file to our node, we need to convert `erc20.wasm` to hex.
    - Command to generate hex file: `xxd -p erc20.wasm | tr -d $'\n' > erc20.hex`

**And below is the script we use deploy contract, get balance and check result balance after transfer amount 1.**

- [erc20.js](./resources/erc20/erc20.js)

**Then we execute script (establish second session into docker)**

```bash
docker exec -it oasis-ssvm bash
```

and

```bash
node ~/code/oasis-runtime/resources/erc20/erc20.js
```

The result should be the same as the following content.

```bash
Web3 is connected.
accounts: ["0x1cCA28600d7491365520B31b466f88647B9839eC","0xB8b3666d8fEa887D97Ab54f571B8E5020c5c8b58"]
ERC-20 contract created at 0xFBe2Ab6ee22DacE9E2CA1cb42C57bF94A32dDd41

contract.balanceOf(0x1cCA28600d7491365520B31b466f88647B9839eC) = 1000
contract.balanceOf(0xB8b3666d8fEa887D97Ab54f571B8E5020c5c8b58) = 0

Transfer 1 token from address(0x1cCA28600d7491365520B31b466f88647B9839eC) to address(0xB8b3666d8fEa887D97Ab54f571B8E5020c5c8b58)

contract.balanceOf(0x1cCA28600d7491365520B31b466f88647B9839eC) = 999
contract.balanceOf(0xB8b3666d8fEa887D97Ab54f571B8E5020c5c8b58) = 1
```

## Our testing node

We've deployed a testing node to demo our runtime at http://oasis-ssvm-demo.secondstate.io:8545/.
