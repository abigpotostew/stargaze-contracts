# Stargaze CosmWasm Contracts

Stargaze smart contracts are written in [CosmWasm](https://cosmwasm.com), a multi-chain smart contracting platform in Rust.

Contracts run in a WASM VM on the [Stargaze Layer 1 blockchain](https://github.com/public-awesome/stargaze).

## SG-721

Stargaze's NFT contract sg721 is a set of optional extensions on top of [cw721-base](https://github.com/CosmWasm/cw-nfts/tree/main/contracts/cw721-base), and conforms to the [cw721 specification](https://github.com/CosmWasm/cw-nfts/tree/main/packages/cw721).

## MINTER

A contract that facilitates primary market vending machine style minting.

## WHITELIST

A contract that manages a list of addresses.

## Running e2e Tests
```
make optimize
make e2etest
```

# DISCLAIMER

STARGAZE CONTRACTS IS PROVIDED “AS IS”, AT YOUR OWN RISK, AND WITHOUT WARRANTIES OF ANY KIND. No developer or entity involved in creating or instantiating Stargaze smart contracts will be liable for any claims or damages whatsoever associated with your use, inability to use, or your interaction with other users of Stargaze, including any direct, indirect, incidental, special, exemplary, punitive or consequential damages, or loss of profits, cryptocurrencies, tokens, or anything else of value. Although Public Awesome, LLC and it's affilliates developed the initial code for Stargaze, it does not own or control the Stargaze network, which is run by a decentralized validator set.
