# Genesis Blocks

This directory contains the genesis blocks which are included into the runtime
during compilation.

## `genesis.json` - Production genesis block

The genesis block contains the following accounts:

* `1c20a5e03c029c24b4e900363479c81f8e8003ea` is the account for the private faucet
  that is used to fund all other accounts. The private key is stored securely and
  is only available to the [private faucet application](https://github.com/oasislabs/private-faucet).

## `genesis_testing.json` - Testing-only genesis block

*NOTE: This genesis block is only used when the runtime is compiled either in
debug mode or with the `benchmark` feature enabled.*

The genesis block contains the following accounts:

* `1c20a5e03c029c24b4e900363479c81f8e8003ea` is the account for the private faucet
  that is used to fund all other accounts. The private key is stored securely and
  is only available to the [private faucet application](https://github.com/oasislabs/private-faucet).

* `7110316b618d20d0c44728ac2a3d683536ea682b` is a test account with the following
  private key: `533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c`.

* `1cca28600d7491365520b31b466f88647b9839ec` is a test account with the following
  private key: `c61675c22aee77da8f6e19444ece45557dc80e1482aa848f541e94e3e5d91179`.