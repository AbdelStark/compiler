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
PROOF_REL_PATH="$(find target/execute -type f -name proof.json | head -n1)"
popd >/dev/null

if [[ -z "$PROOF_REL_PATH" ]]; then
  echo "error: failed to locate proof.json in scarb output" >&2
  exit 1
fi

PROOF_SOURCE_PATH="$DEMO_DIR/$PROOF_REL_PATH"
PROOF_PATH="$ARTIFACTS_DIR/proof.json"
PUBLIC_INPUTS_PATH="$ARTIFACTS_DIR/public_inputs.json"
VERIFICATION_KEY_PATH="$ARTIFACTS_DIR/verification_key.json"

cp "$PROOF_SOURCE_PATH" "$PROOF_PATH"

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
