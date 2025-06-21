#!/usr/bin/env python3
"""
Simplify ONNX model to eliminate Memcpy nodes and optimize for GPU execution.
This removes unnecessary operations that cause CPU-GPU memory transfers.
"""

import os
import sys
import onnx
import onnxsim
import numpy as np
from onnxruntime.transformers import optimizer
from onnxruntime.transformers.fusion_options import FusionOptions
import argparse

def simplify_model(input_path, output_path, check_n=3):
    """
    Simplify ONNX model to remove unnecessary operations.
    
    Args:
        input_path: Path to input ONNX model
        output_path: Path to save simplified model
        check_n: Number of random inputs to check (default: 3)
    """
    print(f"Loading model from: {input_path}")
    model = onnx.load(input_path)
    
    print("Running onnx-simplifier...")
    # Simplify with dynamic input shapes
    try:
        model_simp, check = onnxsim.simplify(
            model,
            check_n=check_n,
            perform_optimization=True,
            skip_fuse_bn=False,
            skip_shape_inference=False,
            dynamic_input_shape=True,  # Important for dynamic batching
            input_shapes={
                "input_ids": ["batch_size", "sequence_length"],
                "attention_mask": ["batch_size", "sequence_length"]
            }
        )
        
        if not check:
            print("WARNING: Simplified model may not be equivalent to original!")
        else:
            print("✓ Model simplification successful and verified")
            
    except Exception as e:
        print(f"Basic simplification failed: {e}")
        print("Trying with more conservative settings...")
        
        # Fallback with more conservative settings
        model_simp, check = onnxsim.simplify(
            model,
            check_n=1,
            perform_optimization=False,
            skip_fuse_bn=True,
            skip_shape_inference=True
        )
    
    # Additional optimization pass to ensure GPU compatibility
    print("Applying GPU-specific optimizations...")
    
    # Create optimizer with fusion options
    fusion_options = FusionOptions('bert')
    fusion_options.enable_attention = True
    fusion_options.enable_layer_norm = True
    fusion_options.enable_gelu = True
    fusion_options.enable_bias_gelu = True
    fusion_options.enable_gelu_approximation = True
    
    # Save temporary file for optimizer
    temp_path = output_path + ".temp"
    onnx.save(model_simp, temp_path)
    
    try:
        # Optimize for GPU execution
        opt_model = optimizer.optimize_model(
            temp_path,
            model_type='bert',
            optimization_options=fusion_options,
            opt_level=2,  # Less aggressive to avoid CPU ops
            use_gpu=True,
            only_onnxruntime=True,  # Only use optimizations compatible with ORT
            float16=False  # Don't convert here, do it separately if needed
        )
        
        # Save optimized model
        opt_model.save_model_to_file(output_path)
        print(f"✓ Saved GPU-optimized model to: {output_path}")
        
    except Exception as e:
        print(f"GPU optimization failed: {e}")
        print("Saving simplified model without additional optimization...")
        onnx.save(model_simp, output_path)
    
    finally:
        # Clean up temp file
        if os.path.exists(temp_path):
            os.remove(temp_path)
    
    # Analyze the model to show what changed
    print("\nModel analysis:")
    print(f"Original model size: {os.path.getsize(input_path) / 1024 / 1024:.2f} MB")
    print(f"Simplified model size: {os.path.getsize(output_path) / 1024 / 1024:.2f} MB")
    
    # Count operations
    original_ops = {}
    for node in model.graph.node:
        op_type = node.op_type
        original_ops[op_type] = original_ops.get(op_type, 0) + 1
    
    simplified_ops = {}
    model_final = onnx.load(output_path)
    for node in model_final.graph.node:
        op_type = node.op_type
        simplified_ops[op_type] = simplified_ops.get(op_type, 0) + 1
    
    print("\nOperation count changes:")
    all_ops = set(original_ops.keys()) | set(simplified_ops.keys())
    for op in sorted(all_ops):
        orig_count = original_ops.get(op, 0)
        simp_count = simplified_ops.get(op, 0)
        if orig_count != simp_count:
            print(f"  {op}: {orig_count} -> {simp_count} ({simp_count - orig_count:+d})")
    
    # Check for problematic operations
    problematic_ops = ['Cast', 'ConstantOfShape', 'Shape', 'Gather', 'Unsqueeze', 'Concat']
    found_problematic = []
    for op in problematic_ops:
        if op in simplified_ops and simplified_ops[op] > 0:
            found_problematic.append(f"{op} ({simplified_ops[op]})")
    
    if found_problematic:
        print(f"\n⚠️  Found operations that might run on CPU: {', '.join(found_problematic)}")
        print("These operations can cause CPU-GPU memory transfers.")
    
    return output_path

def create_pure_gpu_model(input_path, output_dir):
    """
    Create multiple versions of the model optimized for GPU execution.
    """
    os.makedirs(output_dir, exist_ok=True)
    
    base_name = os.path.basename(input_path).replace('.onnx', '')
    
    # 1. Basic simplification
    simple_path = os.path.join(output_dir, f"{base_name}_simplified.onnx")
    print("\n=== Step 1: Basic Simplification ===")
    simplify_model(input_path, simple_path)
    
    # 2. FP16 conversion of simplified model
    print("\n=== Step 2: FP16 Conversion ===")
    try:
        import onnxconverter_common
        from onnxconverter_common import float16
        
        model = onnx.load(simple_path)
        model_fp16 = float16.convert_float_to_float16(
            model,
            keep_io_types=True,
            disable_shape_infer=False,
            op_block_list=['DynamicQuantizeLinear', 'QuantizeLinear']  # Keep these in FP32
        )
        
        fp16_path = os.path.join(output_dir, f"{base_name}_simplified_fp16.onnx")
        onnx.save(model_fp16, fp16_path)
        print(f"✓ Saved FP16 model to: {fp16_path}")
        
    except Exception as e:
        print(f"FP16 conversion failed: {e}")
        fp16_path = None
    
    # 3. Test models with ONNX Runtime
    print("\n=== Step 3: Testing Models ===")
    import onnxruntime as ort
    
    models_to_test = [
        ("Original", input_path),
        ("Simplified", simple_path),
    ]
    
    if fp16_path:
        models_to_test.append(("FP16", fp16_path))
    
    for name, path in models_to_test:
        print(f"\nTesting {name} model...")
        try:
            providers = [
                ('CUDAExecutionProvider', {
                    'device_id': 0,
                    'arena_extend_strategy': 'kNextPowerOfTwo',
                    'cudnn_conv_algo_search': 'EXHAUSTIVE',
                    'do_copy_in_default_stream': True,
                }),
                'CPUExecutionProvider'
            ]
            
            sess_options = ort.SessionOptions()
            sess_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
            sess_options.log_severity_level = 3  # Show warnings
            
            session = ort.InferenceSession(path, sess_options, providers=providers)
            
            # Check which providers are actually being used
            actual_providers = session.get_providers()
            print(f"  Providers: {actual_providers}")
            
            # Get input/output info
            inputs = session.get_inputs()
            outputs = session.get_outputs()
            
            print(f"  Inputs: {[f'{inp.name} {inp.shape}' for inp in inputs]}")
            print(f"  Outputs: {[f'{out.name} {out.shape}' for out in outputs]}")
            
        except Exception as e:
            print(f"  ❌ Failed to load: {e}")
    
    print(f"\n✓ Model optimization complete. Models saved in: {output_dir}")
    print("\nRecommended model for GPU inference:")
    if fp16_path and os.path.exists(fp16_path):
        print(f"  → {fp16_path} (Best performance)")
    else:
        print(f"  → {simple_path} (Simplified FP32)")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Simplify ONNX model for GPU execution")
    parser.add_argument("input", help="Input ONNX model path")
    parser.add_argument("--output-dir", default="simplified_models", help="Output directory")
    
    args = parser.parse_args()
    
    # First install onnxsim if not available
    try:
        import onnxsim
    except ImportError:
        print("Installing onnx-simplifier...")
        os.system(f"{sys.executable} -m pip install onnx-simplifier")
        import onnxsim
    
    create_pure_gpu_model(args.input, args.output_dir)