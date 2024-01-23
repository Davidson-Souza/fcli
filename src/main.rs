use anyhow::{Error, Ok};
use cln_plugin::{Builder, Plugin};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const GET_CHAIN_INFO_HELP: &str = r"
    Returns general information about the chain we are in.

    Result:
        chain: the chain we are in, should be one of bitcoin, testnet, signet or regtest (string)
        headercount: how many headers we know about (number)
        blockcount: how many blocks we have downloaded and validated, should be <= headercount (number)
        ibd: whether we are on Initial Block Download (bool)
";

const SEND_RAW_TRANSACTION_HELP: &str = r"\
    Sends a hex-encoded transaction to be included in the blockchain

    Returns:
        success: whether we did succeed into sending the transaction (bool)
        errmsg: An error, if any (string)
";

const GET_UTXO_OUT_HELP: &str = r"
    Returns the associated UTXO amount and script. It only returns if the UTXO exists
    i.e. it have been created and not spent.

    Returns:
        amount: the amount of satoshis in this UTXO (number)
        script: the locking script for this UTXO (string)
";

const ESTIMATE_FEES_HELP: &str = r"
    Returns the fee needed in order to confirm a transaction in 2, 6, 12, 100 blocks.
    This is returned in sats/KWU.

    Returns:
        feerate_floor: The minimun value accepted to even be accepted to the mempool
        feerates: A list of feerate for different confirmation targets
            2: fee in sats/KWU to confirm in 2 blocks (number)
            6:  fee in sats/KWU to confirm in 6 blocks (number)
            12:  fee in sats/KWU to confirm in 12 blocks (number)
            100:  fee in sats/KWU to confirm in 100 blocks (number)
";

const GET_RAW_BLOCK_BY_HEIGHT_HELP: &str = r"
    Returns the hex-encoded block, given its height

    Returns:
        block: hex-encoded block (string)
";

type FlorestaPlugin = Plugin<Client>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let client = Client::new();

    if let Some(plugin) = Builder::new(tokio::io::stdin(), tokio::io::stdout())
        .rpcmethod("getchaininfo", GET_CHAIN_INFO_HELP, get_blockchain_info)
        .rpcmethod(
            "sendrawtransaction",
            SEND_RAW_TRANSACTION_HELP,
            send_raw_transaction,
        )
        .rpcmethod("getutxout", GET_UTXO_OUT_HELP, getutxout)
        .rpcmethod("estimatefees", ESTIMATE_FEES_HELP, estimate_fees)
        .rpcmethod(
            "getrawblockbyheight",
            GET_RAW_BLOCK_BY_HEIGHT_HELP,
            get_raw_block_by_height,
        )
        .start(client)
        .await?
    {
        let _ = plugin.join().await;
    }
    Ok(())
}

/// Returns the Script and Value for a UTXO if it exists
async fn getutxout(p: FlorestaPlugin, v: serde_json::Value) -> Result<serde_json::Value, Error> {
    let state = p.state();

    let input = v.get("txid").zip(v.get("vout"));

    let (txid, vout) = match input {
        Some((txid, vout)) => (txid, vout),
        _ => return Err(Error::msg("bad request".to_owned())),
    };
    
    let res = rpc_call(&state, "gettxout", format!("{txid}, {vout}")).await?;
    let res = serde_json::from_str::<JsonRpcResult<GetUtxoResult>>(&res)?;

    match res.result {
        Some(res) if res.txout.is_some() => {
            let res = res.txout.unwrap();
            Ok(json!({"amount": res.value, "script": res.script_pubkey }))
        }
        _ => Ok(json!({
            "amout": null,
            "script": null,
        })),
    }
}

/// Publishes a transaction to the chain
async fn send_raw_transaction(
    p: FlorestaPlugin,
    v: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let state = p.state();
    let Some(tx) = v.get("tx") else {
        return Err(Error::msg("bad request".to_owned()));
    };
    let res = rpc_call(&state, "sendrawtransaction", tx.to_string()).await?;
    let res: JsonRpcResult<String> = serde_json::from_str(&res)?;

    match res.error {
        None => Ok(json!({"success": true, "errmsg": null})),
        Some(e) => Ok(json!({"success": false, "errmsg": e})),
    }
}

/// Estimates the fee needed for inclusing in `n` blocks
async fn estimate_fees(
    p: FlorestaPlugin,
    _v: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    Ok(json!({
    "feerate_floor": 1_000,
    "feerates": [
        { "blocks": 2, "feerate": 1_000 },
        { "blocks": 6, "feerate": 1_000 },
        { "blocks": 12, "feerate": 1_000 },
        { "blocks": 100, "feerate": 1_000 }
    ]}))
}

/// Returns general info about our chain
async fn get_blockchain_info(
    p: FlorestaPlugin,
    _v: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let client = p.state();
    let chaininfo = rpc_call(&client, "getblockchaininfo", "".into()).await?;
    let chaininfo = serde_json::from_str::<JsonRpcResult<GetBlockchainInfo>>(&chaininfo)?
        .result
        .unwrap();

    Ok(json!({
        "chain": chaininfo.chain,
        "headercount": chaininfo.height,
        "blockcount": chaininfo.validated,
        "ibd": chaininfo.ibd,
    }))
}

/// Returns a hex-encoded block given a height
async fn get_raw_block_by_height(
    p: FlorestaPlugin,
    v: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let state = p.state();
    let height = v["height"]
        .as_u64()
        .expect("lightningd sent an invalid request");

    let verbosity = 0;

    let block_hash = rpc_call(&state, "getblockhash", format!("{}", height)).await?;
    let block_hash = serde_json::from_str::<JsonRpcResult<String>>(&block_hash)?;

    if block_hash.result.is_none() {
        return Ok(json!({
            "blockhash": null,
            "block": null,
        }));
    }

    let block = rpc_call(
        &state,
        "getblock",
        format!("\"{}\", {}", block_hash.result.as_ref().unwrap(), verbosity),
    )
    .await?;

    let block = serde_json::from_str::<JsonRpcResult<Vec<u8>>>(&block)?;
    if let Some(block) = block.result {
        let block_data = hex::encode(&block);
        return Ok(json!({
            "blockhash": block_hash.result,
            "block": block_data,
        }));
    }

    Ok(json!({
        "blockhash": null,
        "block": null,
    }))
}

// TODO: Move this to the plugin context
static mut IDS: u32 = 0;

/// Performs a json-rpc request to florestad
async fn rpc_call(client: &Client, method: &str, params: String) -> anyhow::Result<String> {
    let request = unsafe {
        format!(
        "{{\"jsonrpc\":\"2.0\", \"id\":{IDS}, \"method\":\"{method}\", \"params\": [{params}]}}",
    )
    };

    unsafe { IDS += 1 };

    let res = client
        .post("http://127.0.0.1:8080")
        .body(request)
        .header("Content-Type", "application/json")
        .send()
        .await?
        .text()
        .await?;

    anyhow::Ok(res)
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResult<Result> {
    jsonrpc: String,
    error: Option<Value>,
    result: Option<Result>,
    id: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct GetUtxoResult {
    txout: Option<TxOut>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TxOut {
    value: u64,
    script_pubkey: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GetBlockchainInfo {
    best_block: String,
    chain: String,
    difficulty: u32,
    height: u32,
    ibd: bool,
    latest_block_time: u32,
    latest_work: String,
    leaf_count: u64,
    progress: f64,
    root_count: u32,
    root_hashes: Vec<String>,
    validated: u32,
}
