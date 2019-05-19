# Multisig transaction CLI

This Command Line Tool allows you to do a transaction from a multisig address.

### Usage

1. Clone repo, cd into directory
2. `cargo build --release`
3. `cd target/release`
4. `./multisig-cli <sendtoaddress> <amount> <WIF>` (WIF is needed to sign multisig transaction)