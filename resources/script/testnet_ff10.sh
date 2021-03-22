#!/bin/bash

export ENTITY_DIR=/home/secondstate/testnet/entity/
export ENTITY_ID=rbUaXSUbCvrZrYiC0H4eLyGVZvrFkQEOrQoZjaDAolE=,bLgW7X3Ke7gUyyzDUjEf9rDvEAMGRa0hb4Vj8jtPOgY=
export GENESIS_JSON=/home/secondstate/testnet/etc/genesis.json
export RUNTIME_ID=000000000000000000000000000000000000000000000000000000000000ff10
export NONCE=0
cd /home/secondstate/testnet/node/
../oasis-node registry runtime gen_register -y \
    --transaction.fee.gas 10000 \
    --transaction.fee.amount 0 \
    --transaction.file register_runtime.tx \
    --transaction.nonce $NONCE \
    --genesis.file $GENESIS_JSON \
    --signer.backend file \
    --signer.dir $ENTITY_DIR \
    --runtime.id $RUNTIME_ID \
    --runtime.kind compute \
    --runtime.executor.group_size 2 \
    --runtime.storage.group_size 2 \
    --runtime.storage.min_write_replication 2 \
    --runtime.admission_policy entity-whitelist \
    --runtime.admission_policy_entity_whitelist $ENTITY_ID \
    --runtime.genesis.state ../etc/oasis_genesis_testnet_ff10.json \
    --runtime.txn_scheduler.flush_timeout 1s \
    --runtime.txn_scheduler.max_batch_size 10000 \
    --runtime.txn_scheduler.max_batch_size_bytes 16mb \
    --runtime.storage.checkpoint_chunk_size 16777216 \
    --runtime.storage.checkpoint_interval 1000 \
    --runtime.storage.checkpoint_num_kept 5
../oasis-node consensus submit_tx --transaction.file register_runtime.tx
rm -f register_runtime.tx
