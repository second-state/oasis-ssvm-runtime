FROM secondstate/oasis-ssvm

ENV OASIS_UNSAFE_SKIP_AVR_VERIFY 1
ENV OASIS_UNSAFE_SKIP_KM_POLICY  1

RUN mkdir -p /root/code
WORKDIR /root/code

RUN git clone https://github.com/oasisprotocol/oasis-core.git --branch v20.7
RUN git clone https://github.com/second-state/oasis-ssvm-runtime.git --branch ssvm

RUN cd /root/code/oasis-ssvm-runtime \
    && rustup target add x86_64-fortanix-unknown-sgx \
    && make -C ../oasis-core \
    && make symlink-artifacts OASIS_CORE_SRC_PATH=../oasis-core \
    && make

RUN mkdir -p /root/code/web3
WORKDIR /root/code/web3
RUN curl -LO https://raw.githubusercontent.com/second-state/oasis-ssvm-runtime/ssvm/resources/erc20/erc20.js \
    && npm install web3 truffle-hdwallet-provider

WORKDIR /root/code/oasis-ssvm-runtime
CMD make run-gateway
