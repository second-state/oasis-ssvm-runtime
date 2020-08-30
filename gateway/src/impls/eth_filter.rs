// Copyright 2015-2018 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Eth Filter RPC implementation

use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};

use anyhow::anyhow;
use common_types::{filter::Filter as EthcoreFilter, ids::BlockId};
use ethereum_types::{H256, U256};
use jsonrpc_core::{
    futures::{future, prelude::*, stream},
    BoxFuture, Result,
};
use lazy_static::lazy_static;
use oasis_core_runtime::common::logger::get_logger;
use parity_rpc::v1::{
    helpers::{errors, limit_logs, PollFilter, PollManager},
    traits::EthFilter,
    types::{BlockNumber, Filter, FilterChanges, Index, Log},
};
use parking_lot::Mutex;
use prometheus::{labels, register_int_counter_vec, IntCounterVec};
use slog::{info, Logger};

use crate::{translator::Translator, util::jsonrpc_error};

// Metrics.
lazy_static! {
    static ref ETH_FILTER_RPC_CALLS: IntCounterVec = register_int_counter_vec!(
        "web3_gateway_eth_filter_rpc_calls",
        "Number of eth_filter API RPC calls",
        &["call"]
    )
    .unwrap();
}

/// Eth filter rpc implementation for a full node.
pub struct EthFilterClient {
    logger: Logger,
    translator: Arc<Translator>,
    polls: Arc<Mutex<PollManager<PollFilter>>>,
}

impl EthFilterClient {
    /// Creates new Eth filter client.
    pub fn new(translator: Arc<Translator>) -> Self {
        EthFilterClient {
            logger: get_logger("gateway/impls/eth_filter"),
            translator,
            polls: Arc::new(Mutex::new(PollManager::new(Default::default()))),
        }
    }
}

impl EthFilter for EthFilterClient {
    fn new_filter(&self, filter: Filter) -> BoxFuture<U256> {
        ETH_FILTER_RPC_CALLS
            .with(&labels! {"call" => "newFilter",})
            .inc();

        let polls = self.polls.clone();
        Box::new(
            self.translator
                .get_latest_block()
                .map_err(jsonrpc_error)
                .map(move |blk| {
                    let mut polls = polls.lock();
                    let block_number = blk.number_u64();
                    let include_pending = filter.to_block == Some(BlockNumber::Pending);
                    let filter = filter.try_into().unwrap();
                    let id = polls.create_poll(PollFilter::Logs {
                        block_number,
                        filter,
                        include_pending,
                        last_block_hash: None,
                        previous_logs: Default::default(),
                    });

                    id.into()
                }),
        )
    }

    fn new_block_filter(&self) -> BoxFuture<U256> {
        ETH_FILTER_RPC_CALLS
            .with(&labels! {"call" => "newBlockFilter",})
            .inc();

        let polls = self.polls.clone();
        Box::new(
            self.translator
                .get_latest_block()
                .map_err(jsonrpc_error)
                .map(move |blk| {
                    let mut polls = polls.lock();
                    // +1, since we don't want to include the current block.
                    let id = polls.create_poll(PollFilter::Block {
                        last_block_number: blk.number_u64() + 1,
                        recent_reported_hashes: VecDeque::with_capacity(
                            PollFilter::MAX_BLOCK_HISTORY_SIZE,
                        ),
                    });

                    id.into()
                }),
        )
    }

    fn new_pending_transaction_filter(&self) -> Result<U256> {
        ETH_FILTER_RPC_CALLS
            .with(&labels! {"call" => "newPendingTransactionFilter",})
            .inc();

        // We don't have pending transactions, so this is a no-op filter.
        let mut polls = self.polls.lock();
        let id = polls.create_poll(PollFilter::PendingTransaction(BTreeSet::new()));
        Ok(id.into())
    }

    fn filter_changes(&self, index: Index) -> BoxFuture<FilterChanges> {
        ETH_FILTER_RPC_CALLS
            .with(&labels! {"call" => "getFilterChanges",})
            .inc();

        let polls = self.polls.clone();
        let translator = self.translator.clone();

        Box::new(
            self.translator
                .get_latest_block()
                .map_err(jsonrpc_error)
                .and_then(move |blk| -> BoxFuture<FilterChanges> {
                    let mut polls = polls.lock();
                    match polls.poll_mut(&index.value()) {
                        None => Box::new(future::err(errors::filter_not_found())),
                        Some(PollFilter::Block {
                            ref mut last_block_number,
                            ref mut recent_reported_hashes,
                        }) => {
                            // TODO: Should we support block range fetch?
                            let updates = Box::new(
                                stream::iter_ok(*last_block_number..=blk.number_u64())
                                    .and_then(move |round| translator.get_block_by_round(round))
                                    .and_then(|blk| match blk {
                                        Some(blk) => Ok(blk),
                                        None => Err(anyhow!("block not found")),
                                    })
                                    .map(|blk| H256::from(blk.hash()))
                                    .collect()
                                    .map_err(jsonrpc_error)
                                    .map(FilterChanges::Hashes),
                            );

                            *last_block_number = blk.number_u64();
                            updates
                        }
                        Some(PollFilter::PendingTransaction(_)) => {
                            // We don't have pending transactions, so this is a no-op filter.
                            Box::new(future::ok(FilterChanges::Hashes(vec![])))
                        }
                        Some(PollFilter::Logs {
                            ref mut block_number,
                            ref mut last_block_hash,
                            ref mut previous_logs,
                            ref filter,
                            include_pending,
                        }) => {
                            // Build appropriate filter.
                            let mut filter: EthcoreFilter = filter.clone().into();
                            filter.from_block = BlockId::Number(*block_number);
                            filter.to_block = BlockId::Latest;

                            // Save the number of the next block as a first block from which
                            // we want to get logs.
                            *block_number = blk.number_u64() + 1;

                            let limit = filter.limit;
                            Box::new(
                                translator
                                    .clone()
                                    .logs(filter)
                                    .map_err(jsonrpc_error)
                                    .map(|logs| logs.into_iter().map(Into::into).collect())
                                    .map(move |logs| limit_logs(logs, limit))
                                    .map(FilterChanges::Logs),
                            )
                        }
                    }
                }),
        )
    }

    fn filter_logs(&self, index: Index) -> BoxFuture<Vec<Log>> {
        ETH_FILTER_RPC_CALLS
            .with(&labels! {"call" => "getFilterLogs",})
            .inc();

        let filter = {
            let mut polls = self.polls.lock();

            match polls.poll(&index.value()) {
                Some(&PollFilter::Logs {
                    ref filter,
                    include_pending,
                    ..
                }) => filter.clone(),
                Some(_) => return Box::new(future::ok(Vec::new())),
                None => return Box::new(future::err(errors::filter_not_found())),
            }
        };

        let filter: EthcoreFilter = filter.into();
        let limit = filter.limit;

        info!(self.logger, "eth_getFilterLogs"; "filter" => ?filter);

        Box::new(
            self.translator
                .clone()
                .logs(filter)
                .map_err(jsonrpc_error)
                .map(|logs| logs.into_iter().map(Into::into).collect())
                .map(move |logs| limit_logs(logs, limit)),
        )
    }

    fn uninstall_filter(&self, index: Index) -> Result<bool> {
        ETH_FILTER_RPC_CALLS
            .with(&labels! {"call" => "uninstallFilter",})
            .inc();

        Ok(self.polls.lock().remove_poll(&index.value()))
    }
}
