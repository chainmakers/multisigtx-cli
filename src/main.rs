extern crate komodo_rpc_client;
extern crate serde_json;
extern crate serde;
extern crate chrono;

use komodo_rpc_client::arguments::address::Address;
use komodo_rpc_client::{Client, AddressUtxos, SerializedRawTransaction, PrivateKey, RawTransaction, Vin};
use komodo_rpc_client::KomodoRpcApi;

use serde::{Deserialize, Serialize};

use std::{env, fs};
use std::convert::AsRef;

use komodo_rpc_client::arguments::P2SHInputSet;
use std::hash::Hash;
use chrono::{DateTime, Utc};

const FCOIN: f64 = 100_000_000.0;

/*

512103127be86a9a59a1ad13c788cd50c5ad0089a1fb05caa11aef6cc19cfb60d8885d2102917ec792638dd7b0822a108ceb53000d9954133a68da960d80e9a9d0a72c7ec252ae

Based on the number of arguments, we can decide what the program should do: create a new tx, or sign a supplied hex.

Inputs to the program in case of new tx:
- toaddress
- amount
- fromaddress
- redeemScript -> we can always
- privkey

Inputs to the program in case of signing an already signed hex:
- hex to sign (maybe get it encoded first)
    after signing:
    - if complete == false, show encoded string
    - if complete == true, show the hex to broadcast
- wif
*/

fn main() {
    // Collect the command line parameters:
    let mut args = env::args().collect::<Vec<_>>();

    match args.len() {
        3 => {
            // sign an existing hex
            sign_hex(args);
        },
        6 => {
            // create a new tx
           create_tx(args);
        },
        _ => panic!("wrong number of arguments") // todo explain what the arguments are in help message
    };
}

fn create_tx(mut args: Vec<String>) {
    let client = komodo_rpc_client::Client::new_komodo_client().unwrap();

    let privkey = args.pop().unwrap();
    let privkey = PrivateKey::from_string(privkey).unwrap();

    let redeem_script = args.pop().unwrap();
    let send_from_address = Address::from(&args.pop().unwrap()).expect("Please enter a valid KMD address");

    let amount: f64 = args.pop().unwrap().parse().expect("Please enter a valid numeric amount to send");
    let amount = (amount * FCOIN) as u64;
    let send_to_address: Address = Address::from(&args.pop().unwrap()).expect("Please enter a valid KMD address");


    let balance: u64 = client.get_address_balance(
        &komodo_rpc_client::arguments::AddressList::from_address(&send_from_address)
    ).unwrap().balance;

    if balance < amount {
        panic!("balance of {} insufficient!", send_from_address.to_string());
    }

    // get the utxos:

    let addressutxos = client.get_address_utxos(
        &komodo_rpc_client::arguments::AddressList::from_address(&send_from_address)
    ).unwrap();

    // Select the utxos needed based on the amount to send:

    let filtered_utxos = filter_utxos(addressutxos, amount);

    // Construct the transaction, including the p2sh inputs since it is a multisig transaction:

    let rawtx = construct_tx(&client, &filtered_utxos, &send_from_address, &send_to_address, amount);
//    dbg!(&rawtx);
    let p2sh_inputs = komodo_rpc_client::arguments::P2SHInputSetBuilder::from(&filtered_utxos)
        .set_redeem_script(redeem_script.to_string())
        .build()
        .unwrap();

    // Finally, sign the transaction, and print the hex to be broadcasted:

    let signedtx = client.sign_raw_transaction_with_key(
        &rawtx, Some(&p2sh_inputs), Some(vec![&privkey]), None
    );

    match signedtx {
        Ok(tx) => {
            if tx.complete {
                println!("./komodo-cli sendrawtransaction {}", tx.to_string())
            } else {
                // at this point, either the WIF is wrong or
                // the hex has not been signed with enough signers.
                //
                // p2sh_inputs
                // signedtx.hex

                let m = IncompletelySignedTx::new(tx.hex.clone(), p2sh_inputs.clone());
                let e = serde_json::to_string(&m).unwrap();

                let now: DateTime<Utc> = Utc::now();
                let now = now.format("%Y_%m_%d-%H:%M:%S").to_string();

                let _ = fs::write(format!("{}.json", now), e);

                println!("Signing has not yet been completed. If no more signers expected, WIF is most likely incorrect.\n\
                If this transaction needs more signers, send this to a next signer: \n{}", tx.hex)

            }
        }
        Err(err) => panic!("Signing went wrong: {}", err)
    }
}

fn sign_hex(mut args: Vec<String>) {


    // based on the hex, i can reconstruct the p2sh inputs:
    let client = komodo_rpc_client::Client::new_komodo_client().unwrap();

    let privkey = args.pop().unwrap();
    let privkey = PrivateKey::from_string(privkey).unwrap();

    let serialized_hex = args.pop().unwrap(); // this is most likely an already signed transaction
    let decoded_raw_tx = client.decode_raw_transaction(serialized_hex.as_str()).unwrap();

    let redeem_script = extract_redeem_script(&decoded_raw_tx);
}

fn extract_redeem_script(tx: &RawTransaction) -> String {
    let vin: &Vin = tx.vin.get(0).unwrap();
    let script_sig = &vin.script_sig;
    let asm = script_sig.asm.as_str();

    // this asm contains the redeemscript. It is always the last part, after a space.
    let redeem_script = asm.split_whitespace().last().expect("redeemscript was not correctly extracted");

    String::from(redeem_script)
}

fn construct_tx(client: &Client, filteredutxos: &AddressUtxos, send_from_address: &Address, send_to_address: &Address, amount: u64) -> SerializedRawTransaction {
    let inputs = komodo_rpc_client::arguments::CreateRawTransactionInputs::from(filteredutxos);
    let mut outputs = komodo_rpc_client::arguments::CreateRawTransactionOutputs::new();
    outputs.add(&send_to_address.to_string(), amount as f64 / FCOIN);

    let mut interest = 0;
    for utxo in &filteredutxos.0 {
        let verbose_tx = client.get_raw_transaction_verbose(
            komodo_rpc_client::TransactionId::from_hex(&utxo.txid).unwrap()).unwrap();

        if let Some(i) = verbose_tx.vout.get(utxo.output_index as usize).unwrap().interest {
            interest += (i * FCOIN) as u64
        }
    };

    // make sure the amount has 8 decimals:
    // ignore dust:
    // only use total amount from filtered utxos
    let filtered_balance = filteredutxos.0.iter().fold(0, |acc, x| acc + x.satoshis);

    let send_back = filtered_balance - amount + interest - 456;

    if send_back > 100 {
        outputs.add(&send_from_address.to_string(), send_back as f64 / FCOIN);
    }

    let mut sertx = client.create_raw_transaction(inputs, outputs).expect("Something went wrong while constructing rawtx");
    sertx.set_locktime();

    sertx
}

fn filter_utxos(mut addressutxos: AddressUtxos, amount: u64) -> AddressUtxos {
    let mut utxos_to_keep: Vec<String> = vec![];
    let mut _amount: i64 = amount as i64;
    let mut utxos = addressutxos.0.clone();

    utxos.sort_by(|a, b| b.satoshis.cmp(&a.satoshis));

    for utxo in utxos {
        if _amount > 0 {
            _amount -= utxo.satoshis as i64;
            utxos_to_keep.push(utxo.txid.clone())
        }
    }

    addressutxos.0 = addressutxos.0.into_iter().filter(|utxo| utxos_to_keep.contains(&utxo.txid)).collect::<Vec<_>>();

    addressutxos
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct IncompletelySignedTx {
    pub(crate) hex: String,
    pub(crate) p2sh: P2SHInputSet
}

impl IncompletelySignedTx {
    pub(crate) fn new(hex: String, p2sh: P2SHInputSet) -> IncompletelySignedTx {
        IncompletelySignedTx {
            hex,
            p2sh
        }
    }
}