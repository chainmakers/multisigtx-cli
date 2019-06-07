extern crate komodo_rpc_client;
extern crate multisig_cli;

use komodo_rpc_client::arguments::address::Address;
use komodo_rpc_client::PrivateKey;

use std::{env};

use std::fs::File;
use std::io::Read;
use multisig_cli::{FCOIN, MultiSignatureTransaction};
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

            let mut msign: multisig_cli::MultiSignatureTransaction = serde_json::from_str(&contents).expect("Something went wrong while decoding JSON");
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