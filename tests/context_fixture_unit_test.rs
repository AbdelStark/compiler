use arkade_compiler::runtime::context_fixture::parse_context_json;
use arkade_compiler::runtime::env::TxContext;

#[test]
fn run_with_context_file_context_fixture_deserialize_minimal_fixture() {
    let fixture = r#"{
        "tx_context": {
            "txid": "0xabcd1234",
            "version": 7
        }
    }"#;

    let context = parse_context_json(fixture, false).expect("fixture should parse");

    assert_eq!(context.tx_hash, hex::decode("abcd1234").unwrap());
    assert_eq!(context.version, 7);
    assert_eq!(context.locktime, 0);
    assert_eq!(context.weight, 0);
    assert_eq!(context.current_input_index, 0);
    assert!(context.inputs.is_empty());
    assert!(context.outputs.is_empty());
    assert!(context.asset_groups.is_empty());
}

#[test]
fn run_with_context_file_context_fixture_deserialize_full_fixture() {
    let fixture = r#"{
        "tx_context": {
            "txid": "deadbeef00",
            "version": 2,
            "locktime": 9,
            "weight": 850,
            "current_input_index": 1,
            "inputs": [
                {
                    "value": 1000,
                    "script_pubkey": "0014aa",
                    "sequence": 11,
                    "outpoint": "0f",
                    "issuance": "aa55",
                    "assets": [
                        {
                            "txid": "0102",
                            "gidx": 3,
                            "amount": 77,
                            "data": "ab",
                            "control": "cd",
                            "metadata_hash": "ef",
                            "asset_id": "1234"
                        }
                    ]
                }
            ],
            "outputs": [
                {
                    "value": 900,
                    "script_pubkey": "0014bb",
                    "nonce": "00ff",
                    "assets": [
                        {
                            "txid": "0a0b",
                            "gidx": 4,
                            "amount": 55,
                            "data": "aa",
                            "control": "bb",
                            "metadata_hash": "cc",
                            "asset_id": "dd"
                        }
                    ]
                }
            ],
            "asset_groups": [
                {
                    "txid": "0c0d",
                    "gidx": 5,
                    "sum_inputs": 10,
                    "sum_outputs": 8,
                    "num_inputs": 1,
                    "num_outputs": 1,
                    "control": "ee",
                    "metadata_hash": "ff",
                    "asset_id": "eeff"
                }
            ]
        }
    }"#;

    let context = parse_context_json(fixture, false).expect("fixture should parse");

    assert_eq!(context.tx_hash, hex::decode("deadbeef00").unwrap());
    assert_eq!(context.version, 2);
    assert_eq!(context.locktime, 9);
    assert_eq!(context.weight, 850);
    assert_eq!(context.current_input_index, 1);
    assert_eq!(context.inputs.len(), 1);
    assert_eq!(context.outputs.len(), 1);
    assert_eq!(context.asset_groups.len(), 1);

    let input = &context.inputs[0];
    assert_eq!(input.value, 1000);
    assert_eq!(input.script_pubkey, hex::decode("0014aa").unwrap());
    assert_eq!(input.sequence, 11);
    assert_eq!(input.outpoint, hex::decode("0f").unwrap());
    assert_eq!(input.issuance, hex::decode("aa55").unwrap());
    assert_eq!(input.assets.len(), 1);
    assert_eq!(input.assets[0].txid, hex::decode("0102").unwrap());
    assert_eq!(input.assets[0].gidx, 3);
    assert_eq!(input.assets[0].amount, 77);

    let output = &context.outputs[0];
    assert_eq!(output.value, 900);
    assert_eq!(output.script_pubkey, hex::decode("0014bb").unwrap());
    assert_eq!(output.nonce, hex::decode("00ff").unwrap());
    assert_eq!(output.assets.len(), 1);
    assert_eq!(output.assets[0].txid, hex::decode("0a0b").unwrap());

    let group = &context.asset_groups[0];
    assert_eq!(group.txid, hex::decode("0c0d").unwrap());
    assert_eq!(group.gidx, 5);
    assert_eq!(group.sum_inputs, 10);
    assert_eq!(group.sum_outputs, 8);
    assert_eq!(group.num_inputs, 1);
    assert_eq!(group.num_outputs, 1);
    assert_eq!(group.control, hex::decode("ee").unwrap());
    assert_eq!(group.metadata_hash, hex::decode("ff").unwrap());
    assert_eq!(group.asset_id, hex::decode("eeff").unwrap());
}

#[test]
fn run_without_context_falls_back_context_fixture_fallback_on_empty_string() {
    let empty = parse_context_json("", false).expect("empty context should fallback");
    let empty_object = parse_context_json("{}", false).expect("empty object should fallback");
    let synthetic = TxContext::sample();

    assert_eq!(empty.tx_hash, synthetic.tx_hash);
    assert_eq!(empty.version, synthetic.version);
    assert_eq!(empty_object.tx_hash, synthetic.tx_hash);
    assert_eq!(empty_object.version, synthetic.version);
}

#[test]
fn context_strict_rejects_unknown_fields_context_fixture_strict_rejects_unknown_field() {
    let fixture = r#"{
        "tx_context": {
            "txid": "abcd1234",
            "rogue_field": 42
        }
    }"#;

    let err =
        parse_context_json(fixture, true).expect_err("strict mode should reject unknown fields");
    assert!(
        err.contains("rogue_field"),
        "error should mention rogue_field, got: {err}"
    );
}

#[test]
fn run_with_context_file_context_fixture_hex_decode_failure() {
    let fixture = r#"{
        "tx_context": {
            "txid": "ZZZZ"
        }
    }"#;

    let err = parse_context_json(fixture, false).expect_err("invalid hex should fail");
    assert!(
        err.to_ascii_lowercase().contains("hex") || err.to_ascii_lowercase().contains("txid"),
        "error should mention invalid txid hex, got: {err}"
    );
}
