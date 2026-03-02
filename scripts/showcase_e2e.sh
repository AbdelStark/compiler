#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONTRACT_SOURCE="$ROOT_DIR/examples/stark_verifier.ark"
CONTRACT_JSON="$ROOT_DIR/examples/stark_verifier.json"
ARTIFACTS_DIR="$ROOT_DIR/examples/stark_demo/artifacts"
PROOF_PATH="$ARTIFACTS_DIR/proof.json"
PUBLIC_INPUTS_PATH="$ARTIFACTS_DIR/public_inputs.json"
VERIFICATION_KEY_PATH="$ARTIFACTS_DIR/verification_key.json"
TAMPERED_PROOF_PATH="$ARTIFACTS_DIR/proof_tampered.json"

echo "[1/5] Generating Cairo1 STWO proof artifacts..."
"$ROOT_DIR/scripts/generate_proof.sh"

echo "[2/5] Compiling Arkade contract with OP_STARK_VERIFY..."
cargo run --quiet --bin arkadec -- "$CONTRACT_SOURCE" --output "$CONTRACT_JSON"

echo "[3/5] Validating contract assembly includes OP_STARK_VERIFY..."
if ! rg -q 'OP_STARK_VERIFY' "$CONTRACT_JSON"; then
  echo "error: contract JSON does not include OP_STARK_VERIFY" >&2
  exit 1
fi

echo "[4/5] Executing OP_STARK_VERIFY with valid proof (expected success)..."
cargo run --quiet --bin stwo_tool -- execute-opcode \
  --proof "$PROOF_PATH" \
  --public-inputs "$PUBLIC_INPUTS_PATH" \
  --verification-key "$VERIFICATION_KEY_PATH"

echo "[5/5] Tampering with proof and executing again (expected failure)..."
perl -0pe 's/("interaction_pow"\s*:\s*)(\d+)/$1 . ($2 + 1)/e' "$PROOF_PATH" > "$TAMPERED_PROOF_PATH"

if cargo run --quiet --bin stwo_tool -- execute-opcode \
  --proof "$TAMPERED_PROOF_PATH" \
  --public-inputs "$PUBLIC_INPUTS_PATH" \
  --verification-key "$VERIFICATION_KEY_PATH"; then
  echo "error: tampered proof was accepted" >&2
  exit 1
fi

echo "Showcase complete: valid proof accepted, tampered proof rejected."
