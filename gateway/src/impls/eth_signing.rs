use ethereum_types::{H160 as H160, H256 as H256, H520 as H520};
use jsonrpc_core::{futures::future, BoxFuture};
use lazy_static::lazy_static;
use parity_rpc::v1::{
    helpers::errors,
    metadata::Metadata,
    traits::EthSigning,
    types::{Bytes, RichRawTransaction, TransactionRequest},
};
use prometheus::{labels, register_int_counter_vec, IntCounterVec};

// Metrics.
lazy_static! {
    static ref ETH_SIGNING_RPC_CALLS: IntCounterVec = register_int_counter_vec!(
        "web3_gateway_eth_signing_rpc_calls",
        "Number of eth_signing API RPC calls",
        &["call"]
    )
    .unwrap();
}

pub struct EthSigningClient {}

impl EthSigningClient {
    pub fn new() -> EthSigningClient {
        EthSigningClient {}
    }
}

impl EthSigning for EthSigningClient {
    type Metadata = Metadata;

    fn sign(&self, _: Metadata, _: H160, _: Bytes) -> BoxFuture<H520> {
        ETH_SIGNING_RPC_CALLS
            .with(&labels! {"call" => "sign",})
            .inc();
        Box::new(future::failed(errors::unsupported("eth_sign is not implemented because the gateway cannot sign transactions. \
            Make sure that the wallet is setup correctly in the client in case transaction signing is expected to happen transparently".to_string(), None)))
    }

    fn send_transaction(&self, _: Metadata, _: TransactionRequest) -> BoxFuture<H256> {
        ETH_SIGNING_RPC_CALLS
            .with(&labels! {"call" => "sendTransaction",})
            .inc();
        Box::new(future::failed(errors::unsupported("eth_sendTransaction is not implemented because the gateway cannot sign transactions. \
            Make sure that the wallet is setup correctly in the client in case transaction signing is expected to happen transparently".to_string(), None)))
    }

    fn sign_transaction(
        &self,
        _: Metadata,
        _: TransactionRequest,
    ) -> BoxFuture<RichRawTransaction> {
        ETH_SIGNING_RPC_CALLS
            .with(&labels! {"call" => "signTransaction",})
            .inc();
        Box::new(future::failed(errors::unsupported("eth_signTransaction is not implemented because the gateway cannot sign transactions. \
            Make sure that the wallet is setup correctly in the client in case transaction signing is expected to happen transparently".to_string(), None)))
    }
}
