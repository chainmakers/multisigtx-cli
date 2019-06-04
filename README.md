# Multisig transaction CLI

This Command Line Tool allows you to do a transaction from a multisig address.

### Usage

1. Clone repo, cd into directory
2. `cargo build --release`
3. `cd target/release`

There are 2 ways of supplying command line arguments:
- to create: `./multisig-cli <sendtoaddress> <amount> <sendfromaddress> <redeemScript> <WIF>` 
- to sign: `./multisig-cli <location of JSON> <WIF>`