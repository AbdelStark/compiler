#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_DIR="$ROOT_DIR/examples/stark_demo"
ARTIFACTS_DIR="$DEMO_DIR/artifacts"

if ! command -v scarb >/dev/null 2>&1; then
  echo "error: scarb is required but not installed" >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required but not installed" >&2
  exit 1
fi

mkdir -p "$DEMO_DIR/src"
mkdir -p "$ARTIFACTS_DIR"

# Keep the executable target in sync with the demo source requested by the repo task.
cp "$DEMO_DIR/fibo.cairo" "$DEMO_DIR/src/lib.cairo"

pushd "$DEMO_DIR" >/dev/null
scarb prove --execute --target bootloader

EXECUTION_ROOT="target/execute/stark_demo"
LATEST_EXECUTION_DIR="$(
  find "$EXECUTION_ROOT" -mindepth 1 -maxdepth 1 -type d -name 'execution*' \
    | awk -F'execution' 'NF > 1 { print $0 "|" $NF }' \
    | sort -t'|' -k2,2n \
    | tail -n1 \
    | cut -d'|' -f1
)"
popd >/dev/null

if [[ -z "$LATEST_EXECUTION_DIR" ]]; then
  echo "error: failed to locate execution directory in scarb output" >&2
  exit 1
fi

PROOF_SOURCE_PATH="$DEMO_DIR/$LATEST_EXECUTION_DIR/proof/proof.json"
PROOF_PATH="$ARTIFACTS_DIR/proof.json"
PUBLIC_INPUTS_PATH="$ARTIFACTS_DIR/public_inputs.json"
VERIFICATION_KEY_PATH="$ARTIFACTS_DIR/verification_key.json"

if [[ ! -f "$PROOF_SOURCE_PATH" ]]; then
  echo "error: expected proof file not found at $PROOF_SOURCE_PATH" >&2
  exit 1
fi

/bin/cp "$PROOF_SOURCE_PATH" "$PROOF_PATH"
if [[ ! -f "$PROOF_PATH" ]]; then
  echo "error: failed to materialize proof artifact at $PROOF_PATH" >&2
  exit 1
fi

cargo run --quiet --bin stwo_tool -- materialize \
  --proof "$PROOF_PATH" \
  --public-inputs "$PUBLIC_INPUTS_PATH" \
  --verification-key "$VERIFICATION_KEY_PATH"

cargo run --quiet --bin stwo_tool -- verify \
  --proof "$PROOF_PATH" \
  --public-inputs "$PUBLIC_INPUTS_PATH" \
  --verification-key "$VERIFICATION_KEY_PATH"

echo "Generated STWO artifacts:"
echo "  proof: $PROOF_PATH"
echo "  public inputs: $PUBLIC_INPUTS_PATH"
echo "  verification key: $VERIFICATION_KEY_PATH"
