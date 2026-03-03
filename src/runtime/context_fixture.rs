use serde::Deserialize;

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

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetEntryWireStrict {
    #[serde(default)]
    txid: String,
    #[serde(default)]
    gidx: u16,
    #[serde(default)]
    amount: i64,
    #[serde(default)]
    data: String,
    #[serde(default)]
    control: String,
    #[serde(default)]
    metadata_hash: String,
    #[serde(default)]
    asset_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TxInputWireStrict {
    #[serde(default)]
    value: i64,
    #[serde(default)]
    script_pubkey: String,
    #[serde(default)]
    sequence: i64,
    #[serde(default)]
    outpoint: String,
    #[serde(default)]
    issuance: String,
    #[serde(default)]
    assets: Vec<AssetEntryWireStrict>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TxOutputWireStrict {
    #[serde(default)]
    value: i64,
    #[serde(default)]
    script_pubkey: String,
    #[serde(default)]
    nonce: String,
    #[serde(default)]
    assets: Vec<AssetEntryWireStrict>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetGroupWireStrict {
    #[serde(default)]
    txid: String,
    #[serde(default)]
    gidx: u16,
    #[serde(default)]
    sum_inputs: i64,
    #[serde(default)]
    sum_outputs: i64,
    #[serde(default)]
    num_inputs: i64,
    #[serde(default)]
    num_outputs: i64,
    #[serde(default)]
    control: String,
    #[serde(default)]
    metadata_hash: String,
    #[serde(default)]
    asset_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TxContextWireStrict {
    #[serde(default)]
    txid: String,
    #[serde(default = "default_version")]
    version: i64,
    #[serde(default)]
    locktime: i64,
    #[serde(default)]
    weight: i64,
    #[serde(default)]
    current_input_index: usize,
    #[serde(default)]
    inputs: Vec<TxInputWireStrict>,
    #[serde(default)]
    outputs: Vec<TxOutputWireStrict>,
    #[serde(default)]
    asset_groups: Vec<AssetGroupWireStrict>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextFixtureStrict {
    tx_context: TxContextWireStrict,
}

fn default_version() -> i64 {
    2
}

impl From<AssetEntryWireStrict> for AssetEntryWire {
    fn from(value: AssetEntryWireStrict) -> Self {
        Self {
            txid: value.txid,
            gidx: value.gidx,
            amount: value.amount,
            data: value.data,
            control: value.control,
            metadata_hash: value.metadata_hash,
            asset_id: value.asset_id,
        }
    }
}

impl From<TxInputWireStrict> for TxInputWire {
    fn from(value: TxInputWireStrict) -> Self {
        Self {
            value: value.value,
            script_pubkey: value.script_pubkey,
            sequence: value.sequence,
            outpoint: value.outpoint,
            issuance: value.issuance,
            assets: value.assets.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<TxOutputWireStrict> for TxOutputWire {
    fn from(value: TxOutputWireStrict) -> Self {
        Self {
            value: value.value,
            script_pubkey: value.script_pubkey,
            nonce: value.nonce,
            assets: value.assets.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<AssetGroupWireStrict> for AssetGroupWire {
    fn from(value: AssetGroupWireStrict) -> Self {
        Self {
            txid: value.txid,
            gidx: value.gidx,
            sum_inputs: value.sum_inputs,
            sum_outputs: value.sum_outputs,
            num_inputs: value.num_inputs,
            num_outputs: value.num_outputs,
            control: value.control,
            metadata_hash: value.metadata_hash,
            asset_id: value.asset_id,
        }
    }
}

impl From<TxContextWireStrict> for TxContextWire {
    fn from(value: TxContextWireStrict) -> Self {
        Self {
            txid: value.txid,
            version: value.version,
            locktime: value.locktime,
            weight: value.weight,
            current_input_index: value.current_input_index,
            inputs: value.inputs.into_iter().map(Into::into).collect(),
            outputs: value.outputs.into_iter().map(Into::into).collect(),
            asset_groups: value.asset_groups.into_iter().map(Into::into).collect(),
        }
    }
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
        let fixture: ContextFixtureStrict = serde_json::from_str(trimmed)
            .map_err(|err| format!("invalid context fixture json: {err}"))?;
        return ContextFixture {
            tx_context: fixture.tx_context.into(),
        }
        .into_tx_context();
    }

    if let Ok(fixture) = serde_json::from_str::<ContextFixture>(trimmed) {
        return fixture.into_tx_context();
    }

    let context: TxContextWire = serde_json::from_str(trimmed)
        .map_err(|err| format!("invalid context fixture json: {err}"))?;
    context.into_runtime()
}
