FROM oasisprotocol/oasis-core-dev:master

RUN mkdir /ssvm \
    && cd ssvm \
    && curl -LO https://github.com/second-state/ssvm-evmc/releases/download/evmc6-0.1.1-rc1/libssvm-evmc.so \
    && curl -LO https://github.com/second-state/rust-ssvm/raw/master/examples/fib.wasm

RUN curl -sL https://deb.nodesource.com/setup_14.x | bash
RUN apt update && apt install -y clang llvm nodejs

RUN mkdir -p /ssvm/web3 \
    && cd /ssvm/web3 \
    && npm install web3 truffle-hdwallet-provider \
    && curl -LO https://raw.githubusercontent.com/second-state/oasis-ssvm-runtime/ssvm/resources/erc20/erc20.js

RUN go get github.com/golang/protobuf/protoc-gen-go

# docker build -t secondstate/oasis-ssvm .
