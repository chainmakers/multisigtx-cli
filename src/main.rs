extern crate komodo_rpc_client;
extern crate serde_json;
extern crate serde;
extern crate chrono;

use komodo_rpc_client::arguments::address::Address;
use komodo_rpc_client::{Client, AddressUtxos, SerializedRawTransaction, PrivateKey, SignedRawTransaction};
use komodo_rpc_client::KomodoRpcApi;

use serde::{Deserialize, Serialize};

use std::{env, fs};

use komodo_rpc_client::arguments::P2SHInputSet;
use chrono::{DateTime, Utc};
use std::fs::File;
use std::io::Read;

const FCOIN: f64 = 100_000_000.0;

/*
Based on the number of arguments, we can decide what the program should do: create a new tx, or sign a supplied hex.

Inputs to the program in case of new tx:
- toaddress
- amount
- fromaddress
- redeemScript
- WIF

Inputs to the program in case of signing an already signed hex:
- location of json containing multisig information
- WIF

todo No self send possible! So no claim of rewards possible!
todo txfee
*/

fn main() {
    // Collect the command line parameters:
    let mut args = env::args().collect::<Vec<_>>();

    // Determine whether to create or sign
    match args.len() {
        3 => {
            // sign an existing hex
            // 1. json file with MultiSignWrapper
            // 2. WIF

            // Sanitize the inputs
            // Since we pop as if it is a stack, we need to start from the end:
            let privkey = args.pop().unwrap();
            let privkey = PrivateKey::from_string(privkey).expect("Unable to parse private key, is WIF correct?");
            let file_name = args.pop().unwrap();

            let mut file = File::open(&file_name).expect(&format!("Could not read file: {}", &file_name));
            let mut contents = String::new();
            file.read_to_string(&mut contents).expect("Something went wrong while reading JSON.");

            let mut msign: MultiSignatureTransaction = serde_json::from_str(&contents).expect("Something went wrong while decoding JSON");
            msign.sign(&privkey);

//            dbg!(&msign);

            msign.is_signing_completed();
        },
        6 => {
            // create a new tx
            // 1. send_to_address
            // 2. amount
            // 3. send_from_address
            // 4. redeemScript
            // 5. WIF

            // Sanitize the inputs
            // Since we pop as if it is a stack, we need to start from the end:
            let privkey = args.pop().unwrap();
            let privkey = PrivateKey::from_string(privkey).expect("Unable to parse private key, is WIF correct?");

            let redeem_script = args.pop().unwrap();

            let send_from_address = Address::from(&args.pop().unwrap()).expect("Please enter a valid KMD address");

            let amount: f64 = args.pop().unwrap().parse().expect("Please enter a valid numeric amount to send");
            let amount = (amount * FCOIN) as u64;

            if amount < 100 {
                panic!("dust");
            }

            let send_to_address: Address = Address::from(&args.pop().unwrap()).expect("Please enter a valid KMD address");

            let msign = MultiSignatureTransaction::create(
                &send_to_address,
                amount,
                &send_from_address,
                &redeem_script,
                &privkey
            );

            msign.is_signing_completed();
        },
        _ => panic!("wrong number of arguments") // todo explain what the arguments are in help message
    };
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct MultiSignatureTransaction {
    pub(crate) signed_tx: SignedRawTransaction,
    pub(crate) p2sh: P2SHInputSet
}

impl MultiSignatureTransaction {
    pub fn create(send_to_address: &Address,
              amount: u64,
              send_from_address: &Address,
              redeem_script: &str,
              privkey: &PrivateKey) -> Self {
        let client = komodo_rpc_client::Client::new_komodo_client().unwrap();

        let balance: u64 = client.get_address_balance(
            &komodo_rpc_client::arguments::AddressList::from_address(&send_from_address)
        ).unwrap().balance;

        dbg!(balance);

        if balance < amount {
            panic!("balance of {} insufficient!", send_from_address.to_string());
        }

        // get the utxos:
        let addressutxos = client.get_address_utxos(
            &komodo_rpc_client::arguments::AddressList::from_address(&send_from_address)
        ).unwrap();

        // Select the utxos needed based on the amount to send:
        let filtered_utxos = filter_utxos(addressutxos, amount);

        // Construct the raw transaction and the p2sh inputs, since it is a multisig transaction:
        let rawtx = construct_tx(&client, &filtered_utxos, &send_from_address, &send_to_address, amount);

        let p2sh_inputs = komodo_rpc_client::arguments::P2SHInputSetBuilder::from(&filtered_utxos)
            .set_redeem_script(redeem_script.to_string())
            .build()
            .unwrap();

        // Finally, sign the transaction:
        let signed_tx = client.sign_raw_transaction_with_key(
            &rawtx,
            Some(&p2sh_inputs),
            Some(vec![&privkey]),
            None
        ).unwrap();

        dbg!(&signed_tx);

        MultiSignatureTransaction {
            signed_tx,
            p2sh: p2sh_inputs
        }
    }

    pub fn is_signing_completed(&self) {
        if self.signed_tx.complete {
            println!("./komodo-cli sendrawtransaction {}", self.signed_tx.to_string())
        } else {
            let serialized_msign = serde_json::to_string(&self).unwrap();

            let now: DateTime<Utc> = Utc::now();
            let file_name = format!("{}.json", now.format("%Y_%m_%d-%H:%M:%S").to_string());

            let _ = fs::write(&file_name, serialized_msign);

            println!("Signing has not yet been completed. If no more signers are expected, WIF is most likely incorrect.\n\
                If this transaction needs more signers, send the following JSON file to the next signer: {}", file_name);
        }
    }

    pub fn sign(&mut self, privkey: &PrivateKey) {
        let client = komodo_rpc_client::Client::new_komodo_client().unwrap();

        let signed_tx = client.sign_raw_transaction_with_key(
            &SerializedRawTransaction::from_hex(self.signed_tx.hex.clone()),
            Some(&self.p2sh),
            Some(vec![&privkey]),
            None
        ).unwrap();

        self.signed_tx = signed_tx;
    }
}

fn construct_tx(client: &Client, filteredutxos: &AddressUtxos, send_from_address: &Address, send_to_address: &Address, amount: u64) -> SerializedRawTransaction {
    let inputs = komodo_rpc_client::arguments::CreateRawTransactionInputs::from(filteredutxos);
    let mut outputs = komodo_rpc_client::arguments::CreateRawTransactionOutputs::new();
    outputs.add(&send_to_address, amount as f64 / FCOIN);

    // todo what if the amount to spend exactly matches amount in filteredutxos, with regards to tx fee?
    // there could be more utxos that the function just didn't filter.
    // 1. add txfee to filteredutxos parameter
    // 2. subtract fee from

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

    let send_back = ((filtered_balance - amount) + interest);

    if send_back > 100 {
        outputs.add(&send_from_address, send_back as f64 / FCOIN);
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