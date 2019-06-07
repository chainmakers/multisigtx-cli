extern crate serde_json;
extern crate serde;
extern crate chrono;

mod multisignature;

pub use multisignature::MultiSignatureTransaction;
pub const FCOIN: f64 = 100_000_000.0;
