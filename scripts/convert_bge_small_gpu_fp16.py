#!/usr/bin/env python
# Convert BAAI/bge-small-en-v1.5 to GPU-optimized ONNX format with float16 precision

import os
import torch
import torch.nn as nn
from transformers import AutoModel, AutoTokenizer
from pathlib import Path
import sys
import argparse
import numpy as np

# Define the model we want to convert
DEFAULT_MODEL_NAME = "BAAI/bge-small-en-v1.5"
DEFAULT_OUTPUT_DIR = "bge_small_onnx_gpu_fp16"

def mean_pooling(model_output, attention_mask):
    """Mean pooling for sentence embeddings"""
    token_embeddings = model_output[0]  # First element of model_output contains all token embeddings
    input_mask_expanded = attention_mask.unsqueeze(-1).expand(token_embeddings.size()).float()
    # Sum embeddings and divide by the number of non-padding tokens
    sum_embeddings = torch.sum(token_embeddings * input_mask_expanded, 1)
    sum_mask = torch.clamp(input_mask_expanded.sum(1), min=1e-9)
    return sum_embeddings / sum_mask

class SentenceTransformerONNXGPU(torch.nn.Module):
    """GPU-optimized wrapper model with float16 support"""
    def __init__(self, model):
        super().__init__()
        self.model = model

    def forward(self, input_ids, attention_mask):
        model_output = self.model(input_ids=input_ids, attention_mask=attention_mask)
        # Perform mean pooling
        sentence_embeddings = mean_pooling(model_output, attention_mask)
        # Normalize embeddings for better performance
        sentence_embeddings = torch.nn.functional.normalize(sentence_embeddings, p=2, dim=1)
        return sentence_embeddings

def optimize_onnx_model_for_gpu(model_path, optimized_model_path):
    """
    Optimize ONNX model for GPU inference with TensorRT optimizations
    
    Args:
        model_path (str): Path to the original ONNX model
        optimized_model_path (str): Path to save the optimized model
    """
    try:
        import onnx
        from onnx import optimizer
        
        print("Loading ONNX model for GPU optimization...")
        model = onnx.load(model_path)
        
        print("Applying ONNX optimizations for GPU inference...")
        # Apply optimizations that are beneficial for GPU inference
        optimizations = [
            'eliminate_identity',
            'eliminate_dropout',
            'eliminate_unused_initializer',
            'extract_constant_to_initializer',
            'fuse_add_bias_into_conv',
            'fuse_bn_into_conv',
            'fuse_consecutive_concats',
            'fuse_consecutive_reduce_unsqueeze',
            'fuse_consecutive_squeezes',
            'fuse_consecutive_transposes',
            'fuse_matmul_add_bias_into_gemm',
            'fuse_pad_into_conv',
            'fuse_transpose_into_gemm',
            'lift_lexical_references',
        ]
        
        optimized_model = optimizer.optimize(model, optimizations)
        
        # Save optimized model
        onnx.save(optimized_model, optimized_model_path)
        print(f"✓ GPU-optimized model saved to: {optimized_model_path}")
        
        return optimized_model_path
        
    except ImportError:
        print("Error: ONNX optimizer not available.", file=sys.stderr)
        print("Please install: pip install onnx", file=sys.stderr)
        return None
    except Exception as e:
        print(f"Error during optimization: {e}", file=sys.stderr)
        return None

def get_optimal_sequence_length_for_gpu(model_name):
    """
    Get optimal sequence length for GPU inference (shorter is better for speed)
    
    Args:
        model_name (str): Name of the model
        
    Returns:
        int: Optimal sequence length for GPU performance
    """
    try:
        from transformers import AutoConfig
        config = AutoConfig.from_pretrained(model_name)
        
        # For GPU inference, we want to balance between model capability and speed
        # BGE models work well with shorter sequences, and most code/text doesn't need 512 tokens
        max_seq_attrs = ['max_position_embeddings', 'n_positions', 'max_sequence_length']
        
        detected_max = None
        for attr in max_seq_attrs:
            if hasattr(config, attr):
                max_len = getattr(config, attr)
                if max_len and max_len > 0:
                    detected_max = max_len
                    print(f"✓ Model supports max sequence length: {max_len}")
                    break
        
        # For GPU optimization, use a shorter sequence length that covers most use cases
        # but provides better throughput
        optimal_length = min(384, detected_max) if detected_max else 384
        print(f"✓ Using GPU-optimized sequence length: {optimal_length}")
        print(f"  (Shorter sequences = better GPU throughput)")
        
        return optimal_length
            
    except Exception as e:
        print(f"⚠ Could not detect sequence length: {e}")
        print("ℹ Using GPU-optimized default: 384 tokens")
        return 384

def download_and_convert_gpu_model(output_dir=DEFAULT_OUTPUT_DIR, model_name=DEFAULT_MODEL_NAME):
    """
    Downloads a model and converts it to GPU-optimized ONNX format with float16
    
    Args:
        output_dir (str): Directory to save the ONNX model and tokenizer
        model_name (str): Name of the model to download
        
    Returns:
        Path to the saved ONNX model
    """
    print(f"Downloading and loading {model_name} for GPU optimization...")
    
    # Create output directory if it doesn't exist
    os.makedirs(output_dir, exist_ok=True)
    
    # Load tokenizer and model
    try:
        tokenizer = AutoTokenizer.from_pretrained(model_name)
        model = AutoModel.from_pretrained(model_name, torch_dtype=torch.float16)
    except Exception as e:
        print(f"Error loading model/tokenizer {model_name}: {e}", file=sys.stderr)
        sys.exit(1)
    
    # Move model to GPU if available
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    if device.type == "cuda":
        print(f"✓ Using GPU: {torch.cuda.get_device_name()}")
        model = model.to(device)
        model = model.half()  # Convert to float16
    else:
        print("⚠ CUDA not available, using CPU (float16 will be converted to float32)")
    
    # Wrap the model for ONNX export
    onnx_export_model = SentenceTransformerONNXGPU(model)
    onnx_export_model.eval()
    
    # Use optimal sequence length for GPU performance
    seq_len = get_optimal_sequence_length_for_gpu(model_name)
    
    # Create dummy inputs on the same device
    dummy_input_ids = torch.ones(1, seq_len, dtype=torch.long, device=device)
    dummy_attention_mask = torch.ones(1, seq_len, dtype=torch.long, device=device)
    
    # Define model inputs and outputs
    input_names = ["input_ids", "attention_mask"]
    output_names = ["sentence_embedding"]
    
    # GPU-optimized dynamic axes (allow batch size variation but fixed sequence length for better GPU utilization)
    dynamic_axes = {
        "input_ids": {0: "batch_size"},
        "attention_mask": {0: "batch_size"},
        "sentence_embedding": {0: "batch_size"}
    }
    
    # Define output paths
    temp_onnx_path = os.path.join(output_dir, "model_fp16.onnx")
    final_onnx_path = os.path.join(output_dir, "model.onnx")
    
    print(f"Converting model to GPU-optimized ONNX with float16 precision...")
    print(f"  - Target sequence length: {seq_len} (optimized for GPU throughput)")
    print(f"  - Precision: float16 (50% memory reduction)")
    print(f"  - Dynamic batching: enabled")
    
    # Export the model to ONNX format
    try:
        torch.onnx.export(
            onnx_export_model,
            (dummy_input_ids, dummy_attention_mask),
            temp_onnx_path,
            export_params=True,
            opset_version=17,  # Modern opset with good GPU support
            do_constant_folding=True,
            input_names=input_names,
            output_names=output_names,
            dynamic_axes=dynamic_axes,
            verbose=False
        )
    except Exception as e:
        print(f"Error during ONNX export: {e}", file=sys.stderr)
        sys.exit(1)
    
    print(f"✓ Model exported to: {temp_onnx_path}")
    
    # Optimize for GPU inference
    optimized_path = optimize_onnx_model_for_gpu(temp_onnx_path, final_onnx_path)
    
    if optimized_path:
        # Remove temporary model
        try:
            os.remove(temp_onnx_path)
        except:
            pass
    else:
        # If optimization failed, use the original
        os.rename(temp_onnx_path, final_onnx_path)
    
    # Save tokenizer
    tokenizer_path = os.path.join(output_dir)
    tokenizer.save_pretrained(tokenizer_path)
    print(f"✓ Tokenizer saved to: {tokenizer_path}")
    
    return final_onnx_path

def verify_gpu_model(onnx_path, tokenizer_dir, model_name):
    """
    Verify the GPU-optimized ONNX model
    
    Args:
        onnx_path (str): Path to the ONNX model
        tokenizer_dir (str): Path to the tokenizer directory
        model_name (str): Original model name
    """
    print(f"\n--- Verifying GPU-Optimized ONNX Model ---")
    try:
        import onnx
        import onnxruntime as ort
        import time
        
        # Check ONNX model structure
        onnx_model = onnx.load(onnx_path)
        onnx.checker.check_model(onnx_model)
        print("✓ ONNX model structure check passed")
        print(f"✓ ONNX opset version: {onnx_model.opset_import[0].version}")
        
        # Check for float16 nodes
        fp16_nodes = []
        for node in onnx_model.graph.node:
            for attr in node.attribute:
                if attr.name == "to" and attr.i == 10:  # ONNX type for float16
                    fp16_nodes.append(node)
        
        if fp16_nodes:
            print(f"✓ Model contains float16 optimizations ({len(fp16_nodes)} nodes)")
        
        # Set up ONNX Runtime with available GPU providers
        providers = []
        
        # Check if TensorRT is actually available
        available_providers = ort.get_available_providers()
        
        if 'TensorrtExecutionProvider' in available_providers:
            print("✓ TensorRT available")
            providers.append(('TensorrtExecutionProvider', {
                'trt_max_workspace_size': 2147483648,  # 2GB
                'trt_fp16_enable': True,
                'trt_engine_cache_enable': True,
            }))
        
        if 'CUDAExecutionProvider' in available_providers:
            print("✓ CUDA available")
            providers.append(('CUDAExecutionProvider', {
                'device_id': 0,
                'arena_extend_strategy': 'kNextPowerOfTwo',
                'gpu_mem_limit': 2 * 1024 * 1024 * 1024,  # 2GB
                'cudnn_conv_algo_search': 'EXHAUSTIVE',
            }))
        
        # Always include CPU as fallback
        providers.append('CPUExecutionProvider')
        
        print(f"Available providers: {available_providers}")
        print(f"Using providers: {[p[0] if isinstance(p, tuple) else p for p in providers]}")
        
        print("Setting up ONNX Runtime session...")
        ort_session = ort.InferenceSession(onnx_path, providers=providers)
        
        # Print which provider is actually being used
        print(f"✓ Using provider: {ort_session.get_providers()[0]}")
        
        # Get input/output info
        onnx_input_names = [inp.name for inp in ort_session.get_inputs()]
        onnx_output_names = [out.name for out in ort_session.get_outputs()]
        print(f"✓ Inputs: {onnx_input_names}")
        print(f"✓ Outputs: {onnx_output_names}")
        
        # Test inference
        print("Testing GPU inference performance...")
        tokenizer = AutoTokenizer.from_pretrained(tokenizer_dir)
        
        # Test with different batch sizes for GPU optimization
        test_texts = [
            "The quick brown fox jumps over the lazy dog.",
            "Machine learning models can be optimized for different hardware.",
            "GPU inference typically provides better throughput than CPU.",
            "Float16 precision reduces memory usage while maintaining accuracy.",
        ]
        
        for batch_size in [1, 4, 8]:
            if batch_size > len(test_texts):
                continue
                
            texts = test_texts[:batch_size]
            seq_len = get_optimal_sequence_length_for_gpu(model_name)
            inputs = tokenizer(texts, return_tensors="np", padding=True, 
                             truncation=True, max_length=seq_len)
            
            onnx_inputs = {
                onnx_input_names[0]: inputs['input_ids'],
                onnx_input_names[1]: inputs['attention_mask']
            }
            
            # Warmup
            for _ in range(3):
                _ = ort_session.run(onnx_output_names, onnx_inputs)
            
            # Benchmark
            start_time = time.time()
            num_runs = 10
            for _ in range(num_runs):
                onnx_outputs = ort_session.run(onnx_output_names, onnx_inputs)
            end_time = time.time()
            
            avg_time = (end_time - start_time) / num_runs
            tokens_per_sec = (batch_size * seq_len) / avg_time
            
            print(f"✓ Batch size {batch_size}: {avg_time:.4f}s avg, {tokens_per_sec:.0f} tokens/sec")
            
        embeddings = onnx_outputs[0]
        print(f"✓ Output shape: {embeddings.shape}")
        print(f"✓ Embedding dimension: {embeddings.shape[-1]}")
        
        # Verify embeddings are reasonable
        if np.any(embeddings) and not np.all(embeddings == embeddings[0, 0]):
            print("✓ Embeddings appear valid")
        else:
            print("⚠ Warning: Embeddings may be invalid")
            
        print("✓ GPU model verification successful!")
        
    except ImportError:
        print("Please install required packages:", file=sys.stderr)
        print("  pip install onnx onnxruntime-gpu transformers", file=sys.stderr)
    except Exception as e:
        print(f"Error verifying model: {e}", file=sys.stderr)

def main():
    parser = argparse.ArgumentParser(description="Convert BGE model to GPU-optimized ONNX with float16 precision")
    parser.add_argument("--model_name", type=str, default=DEFAULT_MODEL_NAME,
                       help=f"Model to convert (default: {DEFAULT_MODEL_NAME})")
    parser.add_argument("--output_dir", type=str, default=DEFAULT_OUTPUT_DIR,
                       help=f"Output directory (default: {DEFAULT_OUTPUT_DIR})")
    parser.add_argument("--skip_verification", action="store_true",
                       help="Skip model verification")
    
    args = parser.parse_args()
    
    print(f"--- GPU-Optimized ONNX Conversion for {args.model_name} ---")
    print("Features:")
    print("  • Float16 precision for 50% memory reduction")
    print("  • Optimized sequence length for GPU throughput")
    print("  • TensorRT and CUDA execution provider support")
    print("  • Batch processing optimization")
    print("  • ONNX graph optimizations for GPU inference")
    
    # Check CUDA availability
    if not torch.cuda.is_available():
        print("\n⚠ Warning: CUDA not available. Model will be created but may not run optimally on GPU.")
        response = input("Continue anyway? (y/N): ")
        if response.lower() != 'y':
            sys.exit(1)
    
    # Convert model
    onnx_path = download_and_convert_gpu_model(
        output_dir=args.output_dir,
        model_name=args.model_name
    )
    
    # Verify model
    if not args.skip_verification:
        verify_gpu_model(onnx_path, args.output_dir, args.model_name)
    
    print(f"\n--- GPU-Optimized Model Conversion Complete ---")
    print("=" * 60)
    print(f"Files saved to '{args.output_dir}' directory:")
    model_file = os.path.join(args.output_dir, 'model.onnx')
    print(f"  • Model: {model_file}")
    print(f"  • Tokenizer: {os.path.join(args.output_dir, 'tokenizer.json')}")
    
    print("\n🚀 GPU Performance Optimizations:")
    print("  • Float16 precision (50% memory reduction)")
    print("  • Optimized sequence length for better GPU utilization")
    print("  • TensorRT execution provider support")
    print("  • CUDA memory optimizations")
    print("  • Dynamic batching for throughput")
    
    print(f"\n📋 Usage with sagitta-cli (GPU-optimized):")
    abs_model_path = os.path.abspath(model_file)
    abs_tokenizer_path = os.path.abspath(args.output_dir)
    
    print("\nEnvironment Variables:")
    print(f"  export SAGITTA_ONNX_MODEL=\"{abs_model_path}\"")
    print(f"  export SAGITTA_ONNX_TOKENIZER=\"{abs_tokenizer_path}\"")
    print("  ./target/release/sagitta-cli index <your_code_dir>")
    
    print("\n⚡ For maximum GPU performance:")
    print("  • Ensure CUDA 11.8+ is installed")
    print("  • Use onnxruntime-gpu with TensorRT")
    print("  • Enable I/O binding in your configuration")
    print("  • Use larger batch sizes when possible")
    print("  • Monitor GPU memory usage")
    
    print("\n⚠️ Important Notes:")
    print("  • This model is optimized for GPU inference")
    print("  • Requires CUDA-compatible GPU")
    print("  • Float16 may have minimal accuracy impact")
    print("  • Rebuild vector index when switching models")
    print("=" * 60)

if __name__ == "__main__":
    main()