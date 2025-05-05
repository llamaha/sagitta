#!/bin/bash
set -e

# This script sets up the quantized ONNX model and tokenizer in ./onnx

MODEL_NAME="flax-sentence-embeddings/st-codesearch-distilroberta-base"
OUTPUT_DIR="./onnx"
QUANTIZED_MODEL_PATH="$OUTPUT_DIR/model_quantized.onnx"

# Run the conversion and quantization
python3 scripts/convert_st_code_model.py \
  --model_name "$MODEL_NAME" \
  --output_dir "$OUTPUT_DIR" \
  --quantize \
  --quantized_model_path "$QUANTIZED_MODEL_PATH"

if [ ! -f "$QUANTIZED_MODEL_PATH" ]; then
  echo "Quantized model was not created!" >&2
  exit 1
fi

# Check for tokenizer.json
if [ ! -f "$OUTPUT_DIR/tokenizer.json" ]; then
  echo "tokenizer.json not found in $OUTPUT_DIR!" >&2
  exit 1
fi

echo "Quantized ONNX model and tokenizer are ready in $OUTPUT_DIR." 