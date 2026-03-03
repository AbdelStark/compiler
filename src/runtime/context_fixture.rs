use serde::Deserialize;
use serde_json::{Map, Value};

use crate::runtime::env::{AssetEntry, AssetGroup, TxContext, TxInput, TxOutput};

#[derive(Debug, Clone, Deserialize)]
pub struct AssetEntryWire {
    #[serde(default)]
    pub txid: String,
    #[serde(default)]
    pub gidx: u16,
    #[serde(default)]
    pub amount: i64,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub control: String,
    #[serde(default)]
    pub metadata_hash: String,
    #[serde(default)]
    pub asset_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxInputWire {
    #[serde(default)]
    pub value: i64,
    #[serde(default)]
    pub script_pubkey: String,
    #[serde(default)]
    pub sequence: i64,
    #[serde(default)]
    pub outpoint: String,
    #[serde(default)]
    pub issuance: String,
    #[serde(default)]
    pub assets: Vec<AssetEntryWire>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxOutputWire {
    #[serde(default)]
    pub value: i64,
    #[serde(default)]
    pub script_pubkey: String,
    #[serde(default)]
    pub nonce: String,
    #[serde(default)]
    pub assets: Vec<AssetEntryWire>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetGroupWire {
    #[serde(default)]
    pub txid: String,
    #[serde(default)]
    pub gidx: u16,
    #[serde(default)]
    pub sum_inputs: i64,
    #[serde(default)]
    pub sum_outputs: i64,
    #[serde(default)]
    pub num_inputs: i64,
    #[serde(default)]
    pub num_outputs: i64,
    #[serde(default)]
    pub control: String,
    #[serde(default)]
    pub metadata_hash: String,
    #[serde(default)]
    pub asset_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxContextWire {
    #[serde(default)]
    pub txid: String,
    #[serde(default = "default_version")]
    pub version: i64,
    #[serde(default)]
    pub locktime: i64,
    #[serde(default)]
    pub weight: i64,
    #[serde(default)]
    pub current_input_index: usize,
    #[serde(default)]
    pub inputs: Vec<TxInputWire>,
    #[serde(default)]
    pub outputs: Vec<TxOutputWire>,
    #[serde(default)]
    pub asset_groups: Vec<AssetGroupWire>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContextFixture {
    pub tx_context: TxContextWire,
}

const BITCOIN_TX_VERSION_V2: i64 = 2;

const ROOT_FIELDS: [&str; 1] = ["tx_context"];
const TX_CONTEXT_FIELDS: [&str; 8] = [
    "txid",
    "version",
    "locktime",
    "weight",
    "current_input_index",
    "inputs",
    "outputs",
    "asset_groups",
];
const TX_INPUT_FIELDS: [&str; 6] = [
    "value",
    "script_pubkey",
    "sequence",
    "outpoint",
    "issuance",
    "assets",
];
const TX_OUTPUT_FIELDS: [&str; 4] = ["value", "script_pubkey", "nonce", "assets"];
const ASSET_GROUP_FIELDS: [&str; 9] = [
    "txid",
    "gidx",
    "sum_inputs",
    "sum_outputs",
    "num_inputs",
    "num_outputs",
    "control",
    "metadata_hash",
    "asset_id",
];
const ASSET_ENTRY_FIELDS: [&str; 7] = [
    "txid",
    "gidx",
    "amount",
    "data",
    "control",
    "metadata_hash",
    "asset_id",
];

fn default_version() -> i64 {
    BITCOIN_TX_VERSION_V2
}

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("invalid context fixture json: expected object at {path}"))
}

fn array<'a>(value: &'a Value, path: &str) -> Result<&'a Vec<Value>, String> {
    value
        .as_array()
        .ok_or_else(|| format!("invalid context fixture json: expected array at {path}"))
}

fn validate_allowed_fields(
    map: &Map<String, Value>,
    allowed_fields: &[&str],
    path: &str,
) -> Result<(), String> {
    for key in map.keys() {
        if !allowed_fields.contains(&key.as_str()) {
            return Err(format!(
                "invalid context fixture json: unknown field '{key}' at {path}"
            ));
        }
    }
    Ok(())
}

fn validate_asset_entry(value: &Value, path: &str) -> Result<(), String> {
    let map = object(value, path)?;
    validate_allowed_fields(map, &ASSET_ENTRY_FIELDS, path)
}

fn validate_assets(value: &Value, path: &str) -> Result<(), String> {
    for (idx, entry) in array(value, path)?.iter().enumerate() {
        validate_asset_entry(entry, &format!("{path}[{idx}]"))?;
    }
    Ok(())
}

fn validate_tx_input(value: &Value, path: &str) -> Result<(), String> {
    let map = object(value, path)?;
    validate_allowed_fields(map, &TX_INPUT_FIELDS, path)?;
    if let Some(assets) = map.get("assets") {
        validate_assets(assets, &format!("{path}.assets"))?;
    }
    Ok(())
}

fn validate_tx_output(value: &Value, path: &str) -> Result<(), String> {
    let map = object(value, path)?;
    validate_allowed_fields(map, &TX_OUTPUT_FIELDS, path)?;
    if let Some(assets) = map.get("assets") {
        validate_assets(assets, &format!("{path}.assets"))?;
    }
    Ok(())
}

fn validate_asset_group(value: &Value, path: &str) -> Result<(), String> {
    let map = object(value, path)?;
    validate_allowed_fields(map, &ASSET_GROUP_FIELDS, path)
}

fn validate_context_fixture(value: &Value) -> Result<(), String> {
    let root = object(value, "$")?;
    validate_allowed_fields(root, &ROOT_FIELDS, "$")?;

    let tx_context = root
        .get("tx_context")
        .ok_or_else(|| "invalid context fixture json: missing field `tx_context`".to_string())?;
    let tx_context_map = object(tx_context, "$.tx_context")?;
    validate_allowed_fields(tx_context_map, &TX_CONTEXT_FIELDS, "$.tx_context")?;

    if let Some(inputs) = tx_context_map.get("inputs") {
        for (idx, input) in array(inputs, "$.tx_context.inputs")?.iter().enumerate() {
            validate_tx_input(input, &format!("$.tx_context.inputs[{idx}]"))?;
        }
    }
    if let Some(outputs) = tx_context_map.get("outputs") {
        for (idx, output) in array(outputs, "$.tx_context.outputs")?.iter().enumerate() {
            validate_tx_output(output, &format!("$.tx_context.outputs[{idx}]"))?;
        }
    }
    if let Some(groups) = tx_context_map.get("asset_groups") {
        for (idx, group) in array(groups, "$.tx_context.asset_groups")?
            .iter()
            .enumerate()
        {
            validate_asset_group(group, &format!("$.tx_context.asset_groups[{idx}]"))?;
        }
    }

    Ok(())
}

fn decode_hex_field(field: &str, raw: &str) -> Result<Vec<u8>, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Ok(Vec::new());
    }

    let normalized = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);

    if normalized.is_empty() {
        return Ok(Vec::new());
    }

    hex::decode(normalized).map_err(|err| format!("invalid hex for field '{field}': {err}"))
}

impl AssetEntryWire {
    fn into_runtime(self) -> Result<AssetEntry, String> {
        Ok(AssetEntry {
            txid: decode_hex_field("txid", &self.txid)?,
            gidx: self.gidx,
            amount: self.amount,
            data: decode_hex_field("data", &self.data)?,
            control: decode_hex_field("control", &self.control)?,
            metadata_hash: decode_hex_field("metadata_hash", &self.metadata_hash)?,
            asset_id: decode_hex_field("asset_id", &self.asset_id)?,
        })
    }
}

impl TxInputWire {
    fn into_runtime(self) -> Result<TxInput, String> {
        Ok(TxInput {
            value: self.value,
            script_pubkey: decode_hex_field("script_pubkey", &self.script_pubkey)?,
            sequence: self.sequence,
            outpoint: decode_hex_field("outpoint", &self.outpoint)?,
            issuance: decode_hex_field("issuance", &self.issuance)?,
            assets: self
                .assets
                .into_iter()
                .map(AssetEntryWire::into_runtime)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TxOutputWire {
    fn into_runtime(self) -> Result<TxOutput, String> {
        Ok(TxOutput {
            value: self.value,
            script_pubkey: decode_hex_field("script_pubkey", &self.script_pubkey)?,
            nonce: decode_hex_field("nonce", &self.nonce)?,
            assets: self
                .assets
                .into_iter()
                .map(AssetEntryWire::into_runtime)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl AssetGroupWire {
    fn into_runtime(self) -> Result<AssetGroup, String> {
        Ok(AssetGroup {
            txid: decode_hex_field("txid", &self.txid)?,
            gidx: self.gidx,
            sum_inputs: self.sum_inputs,
            sum_outputs: self.sum_outputs,
            num_inputs: self.num_inputs,
            num_outputs: self.num_outputs,
            control: decode_hex_field("control", &self.control)?,
            metadata_hash: decode_hex_field("metadata_hash", &self.metadata_hash)?,
            asset_id: decode_hex_field("asset_id", &self.asset_id)?,
        })
    }
}

impl TxContextWire {
    fn into_runtime(self) -> Result<TxContext, String> {
        Ok(TxContext {
            tx_hash: decode_hex_field("txid", &self.txid)?,
            version: self.version,
            locktime: self.locktime,
            weight: self.weight,
            current_input_index: self.current_input_index,
            inputs: self
                .inputs
                .into_iter()
                .map(TxInputWire::into_runtime)
                .collect::<Result<Vec<_>, _>>()?,
            outputs: self
                .outputs
                .into_iter()
                .map(TxOutputWire::into_runtime)
                .collect::<Result<Vec<_>, _>>()?,
            asset_groups: self
                .asset_groups
                .into_iter()
                .map(AssetGroupWire::into_runtime)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl ContextFixture {
    pub fn into_tx_context(self) -> Result<TxContext, String> {
        self.tx_context.into_runtime()
    }
}

pub fn parse_context_json(json: &str, strict: bool) -> Result<TxContext, String> {
    let trimmed = json.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return Ok(TxContext::default());
    }

    if strict {
        let raw: Value = serde_json::from_str(trimmed)
            .map_err(|err| format!("invalid context fixture json: {err}"))?;
        validate_context_fixture(&raw)?;
        let fixture: ContextFixture = serde_json::from_value(raw)
            .map_err(|err| format!("invalid context fixture json: {err}"))?;
        return fixture.into_tx_context();
    }

    if let Ok(fixture) = serde_json::from_str::<ContextFixture>(trimmed) {
        return fixture.into_tx_context();
    }

    let context: TxContextWire = serde_json::from_str(trimmed)
        .map_err(|err| format!("invalid context fixture json: {err}"))?;
    context.into_runtime()
}
