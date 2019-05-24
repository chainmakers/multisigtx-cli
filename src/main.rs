extern crate komodo_rpc_client;

use komodo_rpc_client::arguments::address::Address;
use komodo_rpc_client::{Client, AddressUtxos, SerializedRawTransaction, PrivateKey};
use komodo_rpc_client::KomodoRpcApi;
use std::env;

const FCOIN: f64 = 100_000_000.0;

fn main() {
    let client = komodo_rpc_client::Client::new_komodo_client().unwrap();
    let address = "bWUUUgS2BDgu6HaswhaMddVopdNUsx7mDg";
    let redeem_script = "512103127be86a9a59a1ad13c788cd50c5ad0089a1fb05caa11aef6cc19cfb60d8885d2102917ec792638dd7b0822a108ceb53000d9954133a68da960d80e9a9d0a72c7ec252ae";

    // Collect the command line parameters:

    let mut args = env::args().collect::<Vec<_>>();

    let privkey = args.pop().unwrap();
    let privkey = PrivateKey::from_string(privkey).unwrap();

    let amount: f64 = args.pop().unwrap().parse().expect("Please enter a valid numeric amount to send");
    let amount = (amount * FCOIN) as u64;
    let sendtoaddress: Address = Address::from(&args.pop().unwrap()).expect("Please enter a valid KMD address");

    // Get balance and check if balance is sufficient:

    let balance: u64 = client.get_address_balance(
        &komodo_rpc_client::arguments::AddressList::from(address)
    ).unwrap().balance;

    if balance < amount {
        panic!("balance of {} insufficient!", address);
    }

    // Get current utxos for address to send from:

    let addressutxos = client.get_address_utxos(
        &komodo_rpc_client::arguments::AddressList::from(address)
    ).unwrap();

    // Select the utxos needed:

    let filtered_utxos = filter_utxos(addressutxos, amount);

    // Construct the transaction, including the p2sh inputs since it is a multisig transaction:

    let rawtx = construct_tx(&client, &filtered_utxos, address, &sendtoaddress, amount);
    let p2sh_inputs = komodo_rpc_client::arguments::P2SHInputSetBuilder::from(&filtered_utxos)
        .set_redeem_script(redeem_script.to_string())
        .build()
        .unwrap();

    // Finally, sign the transaction, and print the hex to be broadcasted:

    let signedtx = client.sign_raw_transaction_with_key(
        &rawtx, Some(p2sh_inputs), Some(vec![&privkey]), None
    );

    match signedtx {
        Ok(tx) => {
            if tx.complete {
                println!("./komodo-cli sendrawtransaction {}", tx.to_string())
            } else {
                panic!("Wrong WIF!!! {:?}", tx.errors)
            }
        }
        Err(err) => panic!("Signing went wrong: {}", err)
    }

}

fn construct_tx(client: &Client, filteredutxos: &AddressUtxos, sendfromaddress: &str, sendtoaddress: &Address, amount: u64) -> SerializedRawTransaction {
    let inputs = komodo_rpc_client::arguments::CreateRawTransactionInputs::from(filteredutxos);
    let mut outputs = komodo_rpc_client::arguments::CreateRawTransactionOutputs::new();
    outputs.add(&sendtoaddress.to_string(), amount as f64 / FCOIN);

    let mut interest = 0;
    for utxo in &filteredutxos.0 {
        let verbose_tx = client.get_raw_transaction_verbose(
            komodo_rpc_client::TransactionId::from_hex(&utxo.txid).unwrap()).unwrap();

        interest += (verbose_tx.vout.get(utxo.output_index as usize).unwrap().interest * FCOIN) as u64
    };

    // make sure the amount has 8 decimals:
    // ignore dust:
    // only use total amount from filtered utxos
    let filtered_balance = filteredutxos.0.iter().fold(0, |acc, x| acc + x.satoshis);

    let send_back = filtered_balance - amount + interest - 456;

    if send_back > 100 {
        outputs.add(sendfromaddress, send_back as f64 / FCOIN);
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