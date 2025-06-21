# Sagitta Scripts

## Model Optimization Tool: `model-ctl`

The unified model optimization tool for creating optimized embedding models.

### Prerequisites

Install required Python packages:
```bash
pip install onnx onnxsim onnxruntime onnxconverter-common transformers torch

# For GPU support (optional)
pip install onnxruntime-gpu  # instead of onnxruntime
```

Or use the provided install script:
```bash
./scripts/install-model-ctl-deps.sh
```

### Features
- **GPU optimization**: FP16 precision for maximum throughput
- **CPU optimization**: Uses Qdrant's pre-optimized statically quantized models (when available)
- **Full CPU utilization**: Automatically uses all available CPU cores
- **Single tool**: Replaces all previous conversion scripts
- **Smart fallback**: Falls back to custom quantization or optimized FP32 if needed

### Usage

```bash
# Optimize for GPU (creates model.onnx with FP16)
./model-ctl gpu

# Optimize for CPU (creates model.onnx with S8S8 quantization)
./model-ctl cpu

# Create both GPU and CPU optimized models
./model-ctl all

# Use a different model
./model-ctl gpu --model BAAI/bge-base-en-v1.5

# Specify output directory
./model-ctl all --output-dir /path/to/models

# Clean up old scripts
./model-ctl clean
```

### Options
- `command`: gpu, cpu, all, or clean
- `--model`: Model to optimize (default: BAAI/bge-small-en-v1.5)
- `--output-dir`: Output directory (default: models)
- `--max-sequence-length`: Max sequence length (default: 384)
- `--skip-verify`: Skip model verification

### Output Files

All files are saved in the same directory:

When running single target (`gpu` or `cpu`):
- `model.onnx` - The optimized model
- `tokenizer.json` - Main tokenizer file
- `tokenizer_config.json` - Tokenizer configuration
- `vocab.txt` - Vocabulary file
- `special_tokens_map.json` - Special tokens mapping

When running `all`:
- `model_gpu.onnx` - GPU-optimized model (FP16)
- `model_cpu.onnx` - CPU-optimized model (S8S8 quantized)
- Plus all tokenizer files listed above

### Examples

```bash
# Basic GPU optimization
./model-ctl gpu
export SAGITTA_ONNX_MODEL="$(pwd)/models/model.onnx"
export SAGITTA_ONNX_TOKENIZER="$(pwd)/models"

# Basic CPU optimization with Qdrant's pre-optimized model
./model-ctl cpu
export SAGITTA_ONNX_MODEL="$(pwd)/models/model.onnx"
export SAGITTA_ONNX_TOKENIZER="$(pwd)/models"

# Clean up old conversion scripts
./model-ctl clean
```

### Required Python Packages

- `onnx` - ONNX model format support
- `onnxsim` - Model simplification (note: imports as `onnxsim`, not `onnx-simplifier`)
- `huggingface-hub` - For downloading pre-optimized models from Hugging Face
- `onnxruntime` - ONNX runtime for CPU inference
- `onnxruntime-gpu` - ONNX runtime for GPU inference (optional, replaces `onnxruntime`)
- `onnxconverter-common` - FP16 conversion support
- `transformers` - Hugging Face transformers for model loading
- `torch` - PyTorch for model export

## Other Scripts

- `convert_all_minilm_model.py` - Convert All-MiniLM models (legacy)
- `download_optimized_bge_model.py` - Download pre-optimized models
- `install-model-ctl-deps.sh` - Install all required dependencies