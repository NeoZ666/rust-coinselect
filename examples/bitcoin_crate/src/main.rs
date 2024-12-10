extern crate bitcoin;
extern crate serde;
extern crate serde_derive;
extern crate serde_json;

use bitcoin::{
    absolute::LockTime, transaction, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn,
    TxOut, Txid, Witness,
};
use rand::Rng;
use rust_coinselect::{
    selectcoin::select_coin,
    types::{CoinSelectionOpt, ExcessStrategy, OutputGroup},
};
use serde_derive::Deserialize;
use std::fs;
use std::{collections::HashSet, path::Path, str::FromStr};

// A struct to read and store transaction inputs from the JSON file
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TxInJson {
    txid: String,
    vout: u32,
    script_sig: String,
    sequence: String,
    witness: Vec<String>,
}

// A struct to read and store transaction outputs from the JSON file
#[derive(Deserialize)]
struct TxOutJson {
    value: f64,
    script_pubkey: String,
}

// A struct to read and store transactions from the JSON file
#[derive(Deserialize)]
struct TransactionJson {
    inputs: Vec<TxInJson>,
    outputs: Vec<TxOutJson>,
}

fn read_json_file(file_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Checking if the given path exists
    if !Path::new(file_path).exists() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        )));
    }
    match fs::read_to_string(file_path) {
        Ok(file_content) => Ok(file_content),
        Err(e) => Err(Box::new(e)),
    }
}

fn json_to_transaction(filedata: &str) -> Result<Vec<Transaction>, Box<dyn std::error::Error>> {
    // Parse transaction data from JSON file into Transaction struct of the bitcoin crate
    let tx_json_vec: Vec<TransactionJson> = serde_json::from_str(filedata)?;
    let mut tx_vec: Vec<Transaction> = Vec::new();

    for tx_json in tx_json_vec {
        let mut tx_in_vec: Vec<TxIn> = Vec::new();
        for tx_inp in tx_json.inputs {
            let txid = Txid::from_str(&tx_inp.txid)?;
            let vout = tx_inp.vout;
            let script_signature = ScriptBuf::from_hex(&tx_inp.script_sig)?;
            let nsequence = Sequence::from_hex(&tx_inp.sequence)?;
            // Converting array of strings to slice of bytes
            let witness: Vec<&str> = tx_inp.witness.iter().map(|w| &w[..]).collect();
            // Converting from slice of bytes to Witness object
            let witnessdata = Witness::from_slice(&witness);            
            tx_in_vec.push(TxIn {
                previous_output: OutPoint { txid, vout },
                script_sig: script_signature,
                sequence: nsequence,
                witness: witnessdata,
            });
        }

        let mut tx_out_vec: Vec<TxOut> = Vec::new();
        for tx_op in tx_json.outputs {
            let op_amount = Amount::from_btc(tx_op.value)?;
            let op_script_pubkey = ScriptBuf::from_hex(&tx_op.script_pubkey)?;
            tx_out_vec.push(TxOut {
                value: op_amount,
                script_pubkey: op_script_pubkey,
            });
        }

        tx_vec.push(Transaction {
            version: transaction::Version::TWO,
            lock_time: LockTime::ZERO,
            input: tx_in_vec,
            output: tx_out_vec,
        });
    }

    Ok(tx_vec)
}

fn create_outputgroup(
    tx: Vec<Transaction>,
) -> Result<Vec<OutputGroup>, Box<dyn std::error::Error>> {
    // Create OutputGroup from transaction data
    let mut rng = rand::thread_rng();
    let mut output_group_vec: Vec<OutputGroup> = Vec::new();
    let total_transactions = tx.len();
    let mut unique_numbers: HashSet<u32> = HashSet::new();
    for tx in tx {
        let mut creation_sequence: u32;
        loop {
            creation_sequence = rng.gen_range(0..total_transactions as u32);
            if unique_numbers.insert(creation_sequence) {
                break;
            }
        }
        output_group_vec.push(OutputGroup {
            value: tx.output.iter().map(|op| op.value.to_sat()).sum(),
            weight: tx.total_size() as u32,
            input_count: tx.input.len(),
            creation_sequence: Some(creation_sequence),
        })
    }

    Ok(output_group_vec)
}

fn create_select_options() -> Result<Vec<CoinSelectionOpt>, Box<dyn std::error::Error>> {
    let mut rng = rand::thread_rng();
    let mut coin_select_options_vec: Vec<CoinSelectionOpt> = Vec::new();
    // Creating 5 different options for coin selection
    for _ in 0..5 {
        // Random selection of Excess Strategy
        let excess_strategy = match rng.gen_range(0..3) {
            0 => ExcessStrategy::ToChange,
            1 => ExcessStrategy::ToFee,
            2 => ExcessStrategy::ToRecipient,
            _ => unreachable!(),
        };
        coin_select_options_vec.push(CoinSelectionOpt {
            target_value: rng.gen_range(40000..5000000000i64) as u64,
            target_feerate: rng.gen_range(1.0..5.0) as f32,
            long_term_feerate: Some(rng.gen_range(1..10) as f32),
            min_absolute_fee: rng.gen_range(1..20) as u64,
            base_weight: rng.gen_range(1..30) as u32,
            change_weight: rng.gen_range(5..30) as u32,
            change_cost: rng.gen_range(1..20) as u64,
            avg_input_weight: rng.gen_range(1..10) as u32,
            avg_output_weight: rng.gen_range(1..10) as u32,
            min_change_value: rng.gen_range(100..1000) as u64,
            excess_strategy,
        })
    }
    Ok(coin_select_options_vec)
}

fn perform_select_coin(utxos: Vec<OutputGroup>, coin_select_options_vec: Vec<CoinSelectionOpt>) {
    // Printing information about the UTXOs used for selection
    println!("\nThe total number of UTXOs available: {:?}", utxos.len());
    for (i, utxo) in utxos.iter().enumerate() {
        println!("\nUTXO #:{}", i);
        println!("\nValue:{} sats", utxo.value);
        println!("Weight:{} bytes", utxo.weight);
        println!("No. of Inputs: {}", utxo.input_count);
        println!(
            "Creation Sequence: {:?}",
            utxo.creation_sequence.unwrap_or(0)
        );
    }

    for (_, coin_select_options) in coin_select_options_vec.iter().enumerate().take(5) {
        println!(
            "\nSelecting UTXOs to total: {:?} sats",
            coin_select_options.target_value
        );
        match select_coin(&utxos, &coin_select_options) {
            Ok(selectionoutput) => {
                println!(
                    "Selected utxo index and waste metrics are: {:?}",
                    selectionoutput
                );
            }
            Err(e) => {
                println!("Error performing coin selection: {:?}", e);
            }
        }
    }
}

fn main() {
    // Read and parse transactions from JSON file
    let transactions = match read_json_file("examples/bitcoin_crate/txdata/transactions.json") {
        Ok(filedata) => match json_to_transaction(&filedata) {
            Ok(tx_vec) => tx_vec,
            Err(e) => {
                println!("Error parsing json {:?}", e);
                return;
            }
        },
        Err(e) => {
            println!("Error reading file {:?}", e);
            return;
        }
    };

    // Create UTXOs of type OutputGroup to be passed to coin selection
    let utxos = match create_outputgroup(transactions) {
        Ok(output_group_vec) => output_group_vec,
        Err(e) => {
            println!("Error creating output group {:?}", e);
            return;
        }
    };

    // Create options for coin selection
    let coin_selection_options = match create_select_options() {
        Ok(coin_select_options_vec) => coin_select_options_vec,
        Err(e) => {
            println!("Error creating coin selection options {:?}", e);
            return;
        }
    };

    // Performing coin selection
    perform_select_coin(utxos, coin_selection_options)
}