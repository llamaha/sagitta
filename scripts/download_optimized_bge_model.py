#!/usr/bin/env python
# Download pre-optimized BGE models or create optimized versions with basic ONNX optimizations

import os
import torch
import sys
import argparse
from pathlib import Path

# Pre-optimized model options
OPTIMIZED_MODELS = {
    "bge-small-fp16": {
        "model_id": "BAAI/bge-small-en-v1.5",
        "description": "BGE Small with float16 + basic optimizations",
        "output_dir": "bge_small_optimized",
        "use_fp16": True,
        "dynamic_sequence": True,
    },
    "bge-small-dynamic": {
        "model_id": "BAAI/bge-small-en-v1.5", 
        "description": "BGE Small with dynamic sequence length (reduces padding waste)",
        "output_dir": "bge_small_dynamic",
        "use_fp16": False,
        "dynamic_sequence": True,
    }
}

def mean_pooling(model_output, attention_mask):
    """Mean pooling for sentence embeddings"""
    token_embeddings = model_output[0]
    input_mask_expanded = attention_mask.unsqueeze(-1).expand(token_embeddings.size()).float()
    sum_embeddings = torch.sum(token_embeddings * input_mask_expanded, 1)
    sum_mask = torch.clamp(input_mask_expanded.sum(1), min=1e-9)
    return sum_embeddings / sum_mask

class OptimizedSentenceTransformer(torch.nn.Module):
    """Optimized wrapper for sentence transformers"""
    def __init__(self, model, normalize=True):
        super().__init__()
        self.model = model
        self.normalize = normalize

    def forward(self, input_ids, attention_mask):
        model_output = self.model(input_ids=input_ids, attention_mask=attention_mask)
        sentence_embeddings = mean_pooling(model_output, attention_mask)
        
        if self.normalize:
            sentence_embeddings = torch.nn.functional.normalize(sentence_embeddings, p=2, dim=1)
            
        return sentence_embeddings

def create_optimized_model(model_config):
    """Create optimized ONNX model with basic optimizations"""
    from transformers import AutoModel, AutoTokenizer
    
    model_id = model_config["model_id"]
    output_dir = model_config["output_dir"]
    use_fp16 = model_config["use_fp16"]
    dynamic_sequence = model_config["dynamic_sequence"]
    
    print(f"Creating optimized model: {model_config['description']}")
    print(f"  - Model: {model_id}")
    print(f"  - Float16: {use_fp16}")
    print(f"  - Dynamic sequences: {dynamic_sequence}")
    
    # Create output directory
    os.makedirs(output_dir, exist_ok=True)
    
    # Load model and tokenizer
    tokenizer = AutoTokenizer.from_pretrained(model_id)
    model = AutoModel.from_pretrained(model_id)
    
    if use_fp16:
        model = model.half()
    
    # Wrap model
    onnx_model = OptimizedSentenceTransformer(model, normalize=True)
    onnx_model.eval()
    
    # Determine sequence length strategy
    if dynamic_sequence:
        # Use dynamic sequence length - this reduces padding waste
        max_seq_len = 512  # Still support up to 512
        print(f"  - Max sequence length: {max_seq_len} (dynamic)")
        
        # Dynamic axes allow variable sequence length
        dynamic_axes = {
            "input_ids": {0: "batch_size", 1: "sequence_length"},
            "attention_mask": {0: "batch_size", 1: "sequence_length"},
            "sentence_embedding": {0: "batch_size"}
        }
    else:
        # Fixed sequence length
        max_seq_len = 512
        print(f"  - Fixed sequence length: {max_seq_len}")
        
        dynamic_axes = {
            "input_ids": {0: "batch_size"},
            "attention_mask": {0: "batch_size"},
            "sentence_embedding": {0: "batch_size"}
        }
    
    # Create dummy inputs
    device = next(onnx_model.parameters()).device
    dtype = torch.long
    
    dummy_input_ids = torch.ones(1, max_seq_len, dtype=dtype, device=device)
    dummy_attention_mask = torch.ones(1, max_seq_len, dtype=dtype, device=device)
    
    # Export to ONNX
    model_path = os.path.join(output_dir, "model.onnx")
    
    print(f"  - Exporting to: {model_path}")
    
    torch.onnx.export(
        onnx_model,
        (dummy_input_ids, dummy_attention_mask),
        model_path,
        export_params=True,
        opset_version=17,
        do_constant_folding=True,
        input_names=["input_ids", "attention_mask"],
        output_names=["sentence_embedding"],
        dynamic_axes=dynamic_axes,
        verbose=False
    )
    
    # Apply basic ONNX optimizations (if onnx is available)
    try:
        import onnx
        print("  - Applying basic ONNX optimizations...")
        
        # Load model
        onnx_model = onnx.load(model_path)
        
        # Basic optimizations that don't require external tools
        from onnx import helper, numpy_helper
        
        # Remove unused initializers
        all_inputs = set()
        for node in onnx_model.graph.node:
            all_inputs.update(node.input)
        
        # Keep only used initializers
        used_initializers = []
        for init in onnx_model.graph.initializer:
            if init.name in all_inputs:
                used_initializers.append(init)
        
        # Update model
        del onnx_model.graph.initializer[:]
        onnx_model.graph.initializer.extend(used_initializers)
        
        # Save optimized model
        onnx.save(onnx_model, model_path)
        print("  ✓ Basic optimizations applied")
        
    except ImportError:
        print("  - ONNX not available, skipping optimizations")
    except Exception as e:
        print(f"  - Optimization failed: {e}")
    
    # Save tokenizer
    tokenizer.save_pretrained(output_dir)
    print(f"  ✓ Tokenizer saved to: {output_dir}")
    
    return model_path

def benchmark_model(model_path, tokenizer_dir, dynamic_sequence=True):
    """Benchmark the optimized model"""
    try:
        import onnxruntime as ort
        import time
        from transformers import AutoTokenizer
        
        print(f"\n--- Benchmarking Model ---")
        
        # Setup session with optimal providers
        providers = ['CUDAExecutionProvider', 'CPUExecutionProvider']
        session = ort.InferenceSession(model_path, providers=providers)
        print(f"✓ Using provider: {session.get_providers()[0]}")
        
        # Load tokenizer  
        tokenizer = AutoTokenizer.from_pretrained(tokenizer_dir)
        
        # Test texts of different lengths
        test_cases = [
            ("Short text", "Hello world"),
            ("Medium text", "The quick brown fox jumps over the lazy dog. " * 3),
            ("Long text", "Machine learning and artificial intelligence are transforming software development. " * 8),
        ]
        
        print("\nPadding efficiency test:")
        print("=" * 60)
        
        for name, text in test_cases:
            # Tokenize with different strategies
            if dynamic_sequence:
                # Let tokenizer determine length (minimal padding)
                inputs = tokenizer(text, return_tensors="np", padding=True, truncation=True)
                actual_length = inputs['input_ids'].shape[1]
            else:
                # Fixed 512 length
                inputs = tokenizer(text, return_tensors="np", padding="max_length", 
                                 truncation=True, max_length=512)
                actual_length = 512
            
            # Count actual tokens (non-padding)
            real_tokens = (inputs['input_ids'] != tokenizer.pad_token_id).sum()
            padding_tokens = actual_length - real_tokens
            efficiency = (real_tokens / actual_length) * 100
            
            print(f"{name:12} | Real: {real_tokens:3d} | Padded: {padding_tokens:3d} | Length: {actual_length:3d} | Efficiency: {efficiency:5.1f}%")
            
            # Benchmark inference
            onnx_inputs = {
                'input_ids': inputs['input_ids'],
                'attention_mask': inputs['attention_mask']
            }
            
            # Warmup
            for _ in range(3):
                _ = session.run(None, onnx_inputs)
            
            # Benchmark
            start = time.time()
            for _ in range(10):
                outputs = session.run(None, onnx_inputs)
            avg_time = (time.time() - start) / 10
            
            tokens_per_sec = actual_length / avg_time
            print(f"             | Time: {avg_time:.4f}s | Throughput: {tokens_per_sec:.0f} tokens/sec")
            print("-" * 60)
        
        print("\n✓ Benchmark complete!")
        
    except Exception as e:
        print(f"Benchmark failed: {e}")

def main():
    parser = argparse.ArgumentParser(description="Download or create optimized BGE models")
    parser.add_argument("--model", choices=list(OPTIMIZED_MODELS.keys()), 
                       default="bge-small-dynamic",
                       help="Model variant to create")
    parser.add_argument("--benchmark", action="store_true",
                       help="Run benchmark after creation")
    
    args = parser.parse_args()
    
    model_config = OPTIMIZED_MODELS[args.model]
    
    print(f"Creating optimized model: {args.model}")
    print(f"Description: {model_config['description']}")
    print("=" * 60)
    
    # Create model
    model_path = create_optimized_model(model_config)
    
    # Benchmark if requested
    if args.benchmark:
        benchmark_model(model_path, model_config["output_dir"], 
                       model_config["dynamic_sequence"])
    
    print(f"\n--- Model Ready ---")
    print(f"Files saved to: {model_config['output_dir']}")
    print(f"Model: {os.path.join(model_config['output_dir'], 'model.onnx')}")
    print(f"Tokenizer: {os.path.join(model_config['output_dir'], 'tokenizer.json')}")
    
    abs_model = os.path.abspath(os.path.join(model_config['output_dir'], 'model.onnx'))
    abs_tokenizer = os.path.abspath(model_config['output_dir'])
    
    print(f"\n🚀 Usage:")
    print(f"export SAGITTA_ONNX_MODEL=\"{abs_model}\"")
    print(f"export SAGITTA_ONNX_TOKENIZER=\"{abs_tokenizer}\"")
    
    print(f"\n💡 Key optimization:")
    if model_config["dynamic_sequence"]:
        print("  • Dynamic sequence length reduces padding waste")
        print("  • Short texts use fewer tokens = faster inference")
        print("  • Long texts still supported up to 512 tokens")
    
    if model_config["use_fp16"]:
        print("  • Float16 precision for 50% memory reduction")
    
    print("  • Normalized embeddings for better similarity")
    print("  • ONNX Runtime optimizations")

if __name__ == "__main__":
    main()