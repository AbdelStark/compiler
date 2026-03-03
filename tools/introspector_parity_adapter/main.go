package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"

	arkade "github.com/ArkLabsHQ/introspector/pkg/arkade"
	"github.com/arkade-os/arkd/pkg/ark-lib/asset"
	"github.com/btcsuite/btcd/chaincfg/chainhash"
	"github.com/btcsuite/btcd/txscript"
	"github.com/btcsuite/btcd/wire"
)

var (
	errUnknownOpcode = errors.New("unknown opcode")
	errMissingBind   = errors.New("missing binding")
)

type externalInput struct {
	Name               string                 `json:"name"`
	Asm                []string               `json:"asm"`
	StrictPlaceholders bool                   `json:"strict_placeholders"`
	Bindings           map[string]wireValueIn `json:"bindings"`
}

type wireValueIn struct {
	Type  string `json:"type"`
	Value any    `json:"value"`
}

type wireValueOut struct {
	Type  string `json:"type"`
	Value any    `json:"value"`
}

type externalResult struct {
	Kind           string         `json:"kind"`
	ErrorCode      *string        `json:"error_code,omitempty"`
	FinalMainStack []wireValueOut `json:"final_main_stack"`
	FinalAltStack  []wireValueOut `json:"final_alt_stack"`
	Telemetry      []any          `json:"telemetry"`
}

func main() {
	args := os.Args[1:]
	if len(args) > 0 && args[0] == "--" {
		args = args[1:]
	}
	if len(args) != 1 {
		writeJSON(externalResult{
			Kind:      "runtime_error",
			ErrorCode: strPtr("Usage"),
		})
		os.Exit(2)
	}

	input, err := readInput(args[0])
	if err != nil {
		writeJSON(externalResult{
			Kind:      "runtime_error",
			ErrorCode: strPtr("Json"),
		})
		os.Exit(2)
	}

	script, err := compileScript(input)
	if err != nil {
		code := classifyBuildError(err)
		writeJSON(externalResult{
			Kind:      "runtime_error",
			ErrorCode: &code,
		})
		return
	}

	tx, prevFetcher, inputAmount := sampleTxContext()
	hashCache := txscript.NewTxSigHashes(tx, prevFetcher)

	engine, err := arkade.NewEngine(
		script, tx, 0,
		txscript.NewSigCache(256),
		hashCache,
		inputAmount,
		prevFetcher,
	)
	if err != nil {
		code := classifyRuntimeError(err)
		writeJSON(externalResult{
			Kind:      "runtime_error",
			ErrorCode: &code,
		})
		return
	}
	engine.SetAssetPacket(sampleAssetPacket())

	execErr := engine.Execute()
	mainStack := stackToWire(engine.GetStack())
	altStack := stackToWire(engine.GetAltStack())

	if execErr == nil {
		writeJSON(externalResult{
			Kind:           "script_true",
			FinalMainStack: mainStack,
			FinalAltStack:  altStack,
			Telemetry:      []any{},
		})
		return
	}

	if isScriptFailure(execErr) {
		writeJSON(externalResult{
			Kind:           "script_false",
			FinalMainStack: mainStack,
			FinalAltStack:  altStack,
			Telemetry:      []any{},
		})
		return
	}

	code := classifyRuntimeError(execErr)
	writeJSON(externalResult{
		Kind:           "runtime_error",
		ErrorCode:      &code,
		FinalMainStack: mainStack,
		FinalAltStack:  altStack,
		Telemetry:      []any{},
	})
}

func readInput(path string) (externalInput, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return externalInput{}, fmt.Errorf("read input: %w", err)
	}
	var in externalInput
	if err := json.Unmarshal(raw, &in); err != nil {
		return externalInput{}, fmt.Errorf("parse input: %w", err)
	}
	if in.Bindings == nil {
		in.Bindings = map[string]wireValueIn{}
	}
	return in, nil
}

func compileScript(in externalInput) ([]byte, error) {
	builder := txscript.NewScriptBuilder()

	for _, token := range in.Asm {
		if strings.HasPrefix(token, "OP_") {
			if err := appendOpcode(builder, token); err != nil {
				return nil, err
			}
			continue
		}

		if key, ok := parsePlaceholder(token); ok {
			wireValue, found := in.Bindings[key]
			if !found {
				if in.StrictPlaceholders {
					return nil, fmt.Errorf("%w: %s", errMissingBind, key)
				}
				builder.AddData([]byte(key))
				continue
			}
			if err := appendWireValue(builder, wireValue); err != nil {
				return nil, err
			}
			continue
		}

		if err := appendLiteral(builder, token); err != nil {
			return nil, err
		}
	}

	script, err := builder.Script()
	if err != nil {
		return nil, err
	}
	return script, nil
}

func appendLiteral(builder *txscript.ScriptBuilder, token string) error {
	if strings.EqualFold(token, "true") {
		builder.AddOp(txscript.OP_TRUE)
		return nil
	}
	if strings.EqualFold(token, "false") {
		builder.AddOp(txscript.OP_FALSE)
		return nil
	}

	if v, err := strconv.ParseInt(token, 10, 64); err == nil {
		builder.AddInt64(v)
		return nil
	}

	if isEvenHex(token) {
		buf, err := hex.DecodeString(token)
		if err != nil {
			return err
		}
		builder.AddData(buf)
		return nil
	}

	builder.AddData([]byte(token))
	return nil
}

func appendWireValue(builder *txscript.ScriptBuilder, value wireValueIn) error {
	switch value.Type {
	case "int":
		v, err := asI64(value.Value)
		if err != nil {
			return err
		}
		builder.AddInt64(v)
		return nil
	case "bool":
		v, ok := value.Value.(bool)
		if !ok {
			return fmt.Errorf("bool binding expects boolean value")
		}
		if v {
			builder.AddOp(txscript.OP_TRUE)
		} else {
			builder.AddOp(txscript.OP_FALSE)
		}
		return nil
	case "bytes_hex":
		s, ok := value.Value.(string)
		if !ok {
			return fmt.Errorf("bytes_hex binding expects string value")
		}
		buf, err := hex.DecodeString(s)
		if err != nil {
			return fmt.Errorf("invalid bytes_hex value: %w", err)
		}
		builder.AddData(buf)
		return nil
	case "symbol":
		s, ok := value.Value.(string)
		if !ok {
			return fmt.Errorf("symbol binding expects string value")
		}
		builder.AddData([]byte(s))
		return nil
	default:
		return fmt.Errorf("unsupported binding type: %s", value.Type)
	}
}

func appendOpcode(builder *txscript.ScriptBuilder, op string) error {
	if op == "OP_1NEGATE" {
		builder.AddInt64(-1)
		return nil
	}
	if n, ok := parseOpSmallInt(op); ok {
		builder.AddInt64(int64(n))
		return nil
	}

	switch op {
	case "OP_DUP":
		builder.AddOp(arkade.OP_DUP)
	case "OP_DROP":
		builder.AddOp(arkade.OP_DROP)
	case "OP_NIP":
		builder.AddOp(arkade.OP_NIP)
	case "OP_VERIFY":
		builder.AddOp(arkade.OP_VERIFY)
	case "OP_IF":
		builder.AddOp(arkade.OP_IF)
	case "OP_ELSE":
		builder.AddOp(arkade.OP_ELSE)
	case "OP_ENDIF":
		builder.AddOp(arkade.OP_ENDIF)
	case "OP_EQUAL":
		builder.AddOp(arkade.OP_EQUAL)
	case "OP_EQUALVERIFY":
		builder.AddOp(arkade.OP_EQUALVERIFY)
	case "OP_NOT":
		builder.AddOp(arkade.OP_NOT)
	case "OP_GREATERTHAN":
		builder.AddOp(arkade.OP_GREATERTHAN)
	case "OP_SHA256INITIALIZE":
		builder.AddOp(arkade.OP_SHA256INITIALIZE)
	case "OP_SHA256":
		builder.AddOp(arkade.OP_SHA256)
	case "OP_SHA256UPDATE":
		builder.AddOp(arkade.OP_SHA256UPDATE)
	case "OP_SHA256FINALIZE":
		builder.AddOp(arkade.OP_SHA256FINALIZE)
	case "OP_INSPECTASSETGROUPASSETID":
		builder.AddOp(arkade.OP_INSPECTASSETGROUPASSETID)
	case "OP_FINDASSETGROUPBYASSETID":
		builder.AddOp(arkade.OP_FINDASSETGROUPBYASSETID)
	default:
		return fmt.Errorf("%w: %s", errUnknownOpcode, op)
	}

	return nil
}

func parseOpSmallInt(op string) (int, bool) {
	if !strings.HasPrefix(op, "OP_") {
		return 0, false
	}
	v, err := strconv.Atoi(strings.TrimPrefix(op, "OP_"))
	if err != nil || v < 0 || v > 16 {
		return 0, false
	}
	return v, true
}

func parsePlaceholder(token string) (string, bool) {
	if strings.HasPrefix(token, "<") && strings.HasSuffix(token, ">") && len(token) > 2 {
		return token[1 : len(token)-1], true
	}
	return "", false
}

func sampleTxContext() (*wire.MsgTx, txscript.PrevOutputFetcher, int64) {
	prevOut := wire.OutPoint{
		Hash:  chainhash.Hash{},
		Index: 0,
	}
	inputAmount := int64(100_000)

	tx := wire.NewMsgTx(2)
	tx.AddTxIn(&wire.TxIn{
		PreviousOutPoint: prevOut,
		Sequence:         0xFFFF_FFF0,
	})
	tx.AddTxOut(&wire.TxOut{Value: 99_000, PkScript: []byte{}})
	tx.AddTxOut(&wire.TxOut{Value: 99_001, PkScript: []byte{}})

	witnessProgram := append([]byte{txscript.OP_1, txscript.OP_DATA_32}, make([]byte, 32)...)
	prevFetcher := txscript.NewMultiPrevOutFetcher(map[wire.OutPoint]*wire.TxOut{
		prevOut: {
			Value:    inputAmount,
			PkScript: witnessProgram,
		},
	})
	return tx, prevFetcher, inputAmount
}

func sampleAssetPacket() asset.Packet {
	packet := make(asset.Packet, 0, 4)
	for i := 0; i < 4; i++ {
		txid := hash32(fmt.Sprintf("asset-group-txid-%d", i))
		packet = append(packet, asset.AssetGroup{
			AssetId: &asset.AssetId{
				Txid:  txid,
				Index: uint16(i),
			},
			Outputs: []asset.AssetOutput{
				{
					Type:   asset.AssetOutputTypeLocal,
					Vout:   uint16(i % 2),
					Amount: uint64(4_750 + i),
				},
			},
		})
	}
	return packet
}

func hash32(label string) chainhash.Hash {
	sum := sha256.Sum256([]byte(label))
	var out chainhash.Hash
	copy(out[:], sum[:])
	return out
}

func stackToWire(stack [][]byte) []wireValueOut {
	out := make([]wireValueOut, 0, len(stack))
	for _, item := range stack {
		out = append(out, wireValueOut{
			Type:  "bytes_hex",
			Value: hex.EncodeToString(item),
		})
	}
	return out
}

func asI64(v any) (int64, error) {
	switch n := v.(type) {
	case float64:
		return int64(n), nil
	case int64:
		return n, nil
	case int:
		return int64(n), nil
	case json.Number:
		return n.Int64()
	default:
		return 0, fmt.Errorf("expected integer value")
	}
}

func classifyBuildError(err error) string {
	switch {
	case errors.Is(err, errUnknownOpcode):
		return "UnknownOpcode"
	case errors.Is(err, errMissingBind):
		return "MissingBinding"
	default:
		msg := strings.ToLower(err.Error())
		if strings.Contains(msg, "unbalanced conditional") {
			return "UnbalancedConditional"
		}
		return "ExternalBuildError"
	}
}

func classifyRuntimeError(err error) string {
	msg := strings.ToLower(err.Error())
	switch {
	case strings.Contains(msg, "unbalanced conditional"):
		return "UnbalancedConditional"
	case strings.Contains(msg, "invalid stack operation"):
		return "StackUnderflow"
	default:
		return "ExternalRuntimeError"
	}
}

func isScriptFailure(err error) bool {
	msg := strings.ToLower(err.Error())
	return strings.Contains(msg, "verify failed") ||
		strings.Contains(msg, "failed verify operation") ||
		strings.Contains(msg, "false stack entry") ||
		strings.Contains(msg, "equalverify") ||
		strings.Contains(msg, "numequalverify") ||
		strings.Contains(msg, "checksigverify") ||
		strings.Contains(msg, "checkmultisigverify") ||
		strings.Contains(msg, "nullfail")
}

func isEvenHex(s string) bool {
	if s == "" || len(s)%2 != 0 {
		return false
	}
	for _, r := range s {
		if !((r >= '0' && r <= '9') || (r >= 'a' && r <= 'f') || (r >= 'A' && r <= 'F')) {
			return false
		}
	}
	return true
}

func strPtr(v string) *string {
	return &v
}

func writeJSON(v any) {
	enc := json.NewEncoder(os.Stdout)
	enc.SetEscapeHTML(false)
	_ = enc.Encode(v)
}
