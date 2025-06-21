#!/usr/bin/env python3
"""
Optimize BGE model for GPU inference with proper memory allocation.
This script creates an ONNX model optimized for GPU execution with:
- GPU memory allocation
- Optimized graph for inference
- FP16 precision for faster computation
- Proper IO binding configuration
"""

import os
import sys
import torch
import onnx
import onnxruntime as ort
from transformers import AutoModel, AutoTokenizer
import numpy as np
from onnxruntime.quantization import quantize_dynamic, QuantType
from onnxruntime.transformers import optimizer
from onnxruntime.transformers.fusion_options import FusionOptions
import argparse

DEFAULT_MODEL = "BAAI/bge-small-en-v1.5"
DEFAULT_OUTPUT = "bge_gpu_optimized"

def create_optimized_model(model_name=DEFAULT_MODEL, output_dir=DEFAULT_OUTPUT, fp16=True):
    """Create GPU-optimized ONNX model with proper settings."""
    
    print(f"Loading model: {model_name}")
    model = AutoModel.from_pretrained(model_name)
    tokenizer = AutoTokenizer.from_pretrained(model_name)
    
    # Ensure output directory exists
    os.makedirs(output_dir, exist_ok=True)
    
    # Set model to eval mode
    model.eval()
    
    # Create dummy input with dynamic axes
    dummy_input = tokenizer(
        "This is a sample text for model export",
        padding="max_length",
        max_length=512,
        truncation=True,
        return_tensors="pt"
    )
    
    # Export to ONNX with dynamic axes for batch size and sequence length
    onnx_path = os.path.join(output_dir, "model.onnx")
    print(f"Exporting to ONNX: {onnx_path}")
    
    # Dynamic axes for true dynamic batching
    dynamic_axes = {
        'input_ids': {0: 'batch_size', 1: 'sequence_length'},
        'attention_mask': {0: 'batch_size', 1: 'sequence_length'},
        'last_hidden_state': {0: 'batch_size', 1: 'sequence_length'}
    }
    
    torch.onnx.export(
        model,
        (dummy_input['input_ids'], dummy_input['attention_mask']),
        onnx_path,
        input_names=['input_ids', 'attention_mask'],
        output_names=['last_hidden_state'],
        dynamic_axes=dynamic_axes,
        opset_version=14,
        do_constant_folding=True,
        export_params=True
    )
    
    # Load and optimize the ONNX model
    print("Optimizing ONNX model for GPU...")
    model_onnx = onnx.load(onnx_path)
    
    # Create optimization options
    fusion_options = FusionOptions('bert')
    fusion_options.enable_attention = True
    fusion_options.enable_layer_norm = True
    fusion_options.enable_gelu = True
    fusion_options.enable_bias_gelu = True
    fusion_options.enable_gelu_approximation = True
    
    # Optimize model
    optimized_model = optimizer.optimize_model(
        onnx_path,
        model_type='bert',
        optimization_options=fusion_options,
        opt_level=99,  # Maximum optimization
        use_gpu=True,
        only_onnxruntime=False
    )
    
    # Save optimized model
    optimized_path = os.path.join(output_dir, "model_optimized.onnx")
    optimized_model.save_model_to_file(optimized_path)
    
    if fp16:
        print("Converting to FP16 for GPU...")
        # Convert to FP16 for GPU
        import onnxconverter_common
        from onnxconverter_common import float16
        
        model_fp16 = float16.convert_float_to_float16(
            optimized_model.model,
            keep_io_types=True,
            disable_shape_infer=False
        )
        
        fp16_path = os.path.join(output_dir, "model_fp16.onnx")
        onnx.save(model_fp16, fp16_path)
        print(f"Saved FP16 model to: {fp16_path}")
        
        final_model_path = fp16_path
    else:
        final_model_path = optimized_path
    
    # Test the model with GPU execution provider
    print("\nTesting GPU inference...")
    providers = [
        ('CUDAExecutionProvider', {
            'device_id': 0,
            'arena_extend_strategy': 'kNextPowerOfTwo',
            'gpu_mem_limit': 4 * 1024 * 1024 * 1024,  # 4GB
            'cudnn_conv_algo_search': 'EXHAUSTIVE',
            'do_copy_in_default_stream': True,
            'cudnn_conv_use_max_workspace': True,
            # 'enable_cuda_graph': True,  # Disabled due to CPU ops in the graph
        }),
        'CPUExecutionProvider'  # Fallback for ops that can't run on GPU
    ]
    
    # Create session options for optimal GPU performance
    sess_options = ort.SessionOptions()
    sess_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    sess_options.enable_mem_pattern = True
    sess_options.enable_mem_reuse = True
    sess_options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
    
    # Create inference session
    session = ort.InferenceSession(final_model_path, sess_options, providers=providers)
    
    # Verify GPU is being used
    print(f"Execution providers: {session.get_providers()}")
    
    # Test inference with different batch sizes
    test_texts = [
        "Short text",
        "Medium length text that contains more tokens",
        "This is a longer text that will test the model's ability to handle various sequence lengths efficiently"
    ]
    
    for batch_size in [1, 4, 8, 16]:
        texts = test_texts[:min(batch_size, len(test_texts))] * (batch_size // len(test_texts) + 1)
        texts = texts[:batch_size]
        
        inputs = tokenizer(
            texts,
            padding=True,  # Dynamic padding
            truncation=True,
            max_length=512,
            return_tensors="np"
        )
        
        # Create IO binding for optimal GPU performance
        io_binding = session.io_binding()
        
        # Bind inputs
        input_ids = inputs['input_ids'].astype(np.int64)
        attention_mask = inputs['attention_mask'].astype(np.int64)
        
        # Pre-allocate output on GPU
        output_shape = (batch_size, input_ids.shape[1], 384)  # 384 is hidden size for bge-small
        
        # For dynamic shapes, we need to check if CUDA is available
        try:
            # Create OrtValue for inputs
            input_ids_ortvalue = ort.OrtValue.ortvalue_from_numpy(input_ids, 'cuda', 0)
            attention_mask_ortvalue = ort.OrtValue.ortvalue_from_numpy(attention_mask, 'cuda', 0)
            
            io_binding.bind_ortvalue_input('input_ids', input_ids_ortvalue)
            io_binding.bind_ortvalue_input('attention_mask', attention_mask_ortvalue)
            io_binding.bind_output('last_hidden_state', 'cuda', 0)
            
            # Run inference
            import time
            start = time.time()
            session.run_with_iobinding(io_binding)
            gpu_time = time.time() - start
            
            outputs = io_binding.copy_outputs_to_cpu()
            
            print(f"Batch size: {batch_size}, Sequence length: {input_ids.shape[1]}, "
                  f"GPU inference time: {gpu_time*1000:.2f}ms")
        except Exception as e:
            # Fallback to standard inference if IO binding fails
            print(f"IO binding failed for batch {batch_size}, using standard inference: {str(e)}")
            
            import time
            start = time.time()
            outputs = session.run(None, {
                'input_ids': input_ids,
                'attention_mask': attention_mask
            })
            cpu_time = time.time() - start
            
            print(f"Batch size: {batch_size}, Sequence length: {input_ids.shape[1]}, "
                  f"Standard inference time: {cpu_time*1000:.2f}ms")
    
    # Save tokenizer config
    tokenizer.save_pretrained(output_dir)
    
    # Create metadata file
    metadata = {
        "model_name": model_name,
        "hidden_size": 384,
        "max_sequence_length": 512,
        "optimization": {
            "fp16": fp16,
            "gpu_optimized": True,
            "dynamic_batching": True,
            "io_binding_ready": True
        },
        "recommended_batch_sizes": [1, 4, 8, 16, 32],
        "files": {
            "model": os.path.basename(final_model_path),
            "tokenizer": "tokenizer_config.json"
        }
    }
    
    import json
    with open(os.path.join(output_dir, "metadata.json"), "w") as f:
        json.dump(metadata, f, indent=2)
    
    print(f"\nOptimized model saved to: {output_dir}")
    print("Optimization complete!")
    
    return output_dir

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Create GPU-optimized BGE ONNX model")
    parser.add_argument("--model", default=DEFAULT_MODEL, help="Hugging Face model name")
    parser.add_argument("--output", default=DEFAULT_OUTPUT, help="Output directory")
    parser.add_argument("--fp32", action="store_true", help="Use FP32 instead of FP16")
    
    args = parser.parse_args()
    
    create_optimized_model(args.model, args.output, fp16=not args.fp32)