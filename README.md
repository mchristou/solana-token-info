# Solana Token Information

This Rust project provides a simple way to retrieve and display information about tokens on the Solana blockchain's mainnet.
It fetches token metadata, some account information and additional data from a metadata URI.

### Installation

Clone the repository and build the project:

```bash
git clone https://github.com/mchristou/solana-token-info.git
cd solana-token-info
cargo build --release
```

### Usage

You can retrieve information for specific token public keys by running the following command:

For one token

```bash
cargo run --release --bin token_info <TOKEN_PUBKEY>
```

For multiple tokens

```bash
cargo run --release --bin token_info <TOKEN1_PUBKEY> <TOKEN2_PUBKEY> <TOKEN3_PUBKEY>
```
