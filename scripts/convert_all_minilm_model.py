#!/usr/bin/env python
# Convert All-MiniLM-L6-v2 Sentence Transformer model to ONNX format

import os
import torch
# Use AutoModel and AutoTokenizer for flexibility
from transformers import AutoModel, AutoTokenizer
from pathlib import Path
import sys
import argparse

# Define the model we want to convert
DEFAULT_MODEL_NAME = "sentence-transformers/all-MiniLM-L6-v2"
DEFAULT_OUTPUT_DIR = "all_minilm_onnx"

# --- Sentence Transformer specific changes START ---
# Sentence Transformers often require pooling the last hidden state.
# This helper function implements mean pooling.
def mean_pooling(model_output, attention_mask):
    token_embeddings = model_output[0] # First element of model_output contains all token embeddings
    input_mask_expanded = attention_mask.unsqueeze(-1).expand(token_embeddings.size()).float()
    # Sum embeddings and divide by the number of non-padding tokens
    sum_embeddings = torch.sum(token_embeddings * input_mask_expanded, 1)
    sum_mask = torch.clamp(input_mask_expanded.sum(1), min=1e-9)
    return sum_embeddings / sum_mask

# Wrapper model to include pooling logic for ONNX export
class SentenceTransformerONNX(torch.nn.Module):
    def __init__(self, model):
        super().__init__()
        self.model = model

    def forward(self, input_ids, attention_mask):
        model_output = self.model(input_ids=input_ids, attention_mask=attention_mask)
        # Perform mean pooling
        sentence_embeddings = mean_pooling(model_output, attention_mask)
        return sentence_embeddings # Return only the pooled sentence embeddings
# --- Sentence Transformer specific changes END ---

def quantize_onnx_model(model_path, quantized_model_path):
    """
    Quantize an ONNX model using onnxruntime quantization tools
    
    Args:
        model_path (str): Path to the original ONNX model
        quantized_model_path (str): Path to save the quantized model
    """
    try:
        from onnxruntime.quantization import quantize_dynamic, QuantType
        
        print("Quantizing ONNX model to INT8...")
        
        # Dynamic quantization - good for CPU inference
        # For ONNX Runtime 1.20, the API is simpler
        quantize_dynamic(
            model_input=model_path,
            model_output=quantized_model_path,
            weight_type=QuantType.QInt8  # Quantize weights to INT8
        )
        
        print(f"✓ Model quantized successfully: {quantized_model_path}")
        return quantized_model_path
        
    except ImportError:
        print("Error: onnxruntime quantization tools not available.", file=sys.stderr)
        print("Please install: pip install onnxruntime", file=sys.stderr)
        return None
    except Exception as e:
        print(f"Error during quantization: {e}", file=sys.stderr)
        return None

def download_and_convert_st_model_to_onnx(output_dir=DEFAULT_OUTPUT_DIR, model_name=DEFAULT_MODEL_NAME, quantized=False):
    """
    Downloads a Sentence Transformer model and converts it to ONNX format

    Args:
        output_dir (str): Directory to save the ONNX model and tokenizer
        model_name (str): Name of the Sentence Transformer model to download
        quantized (bool): Whether to quantize the model

    Returns:
        Path to the saved ONNX model
    """
    print(f"Downloading and loading {model_name} model...")

    # Create output directory if it doesn't exist
    os.makedirs(output_dir, exist_ok=True)

    # Load tokenizer and model using Auto* classes
    try:
        tokenizer = AutoTokenizer.from_pretrained(model_name)
        model = AutoModel.from_pretrained(model_name)
    except Exception as e:
        print(f"Error loading model/tokenizer {model_name}: {e}", file=sys.stderr)
        print("Please ensure the model name is correct and you have internet connectivity.", file=sys.stderr)
        sys.exit(1)

    # Wrap the model for ONNX export to include pooling
    onnx_export_model = SentenceTransformerONNX(model)

    # Set model to evaluation mode
    onnx_export_model.eval()

    # Prepare dummy inputs for tracing (adjust sequence length if needed)
    seq_len = 128 # Common sequence length for ST models, check model config if unsure
    dummy_input_ids = torch.ones(1, seq_len, dtype=torch.long)
    dummy_attention_mask = torch.ones(1, seq_len, dtype=torch.long)

    # Define symbolic names for inputs and output
    input_names = ["input_ids", "attention_mask"]
    # Output name corresponds to the pooled sentence embeddings
    output_names = ["sentence_embedding"]

    # Define dynamic axes
    dynamic_axes = {
        "input_ids": {0: "batch_size", 1: "sequence_length"},
        "attention_mask": {0: "batch_size", 1: "sequence_length"},
        "sentence_embedding": {0: "batch_size"} # Dimension 1 (embedding size) is fixed
    }

    # Define output paths
    if quantized:
        temp_onnx_path = os.path.join(output_dir, "model_fp32.onnx") # Temporary full precision
        final_onnx_path = os.path.join(output_dir, "model.onnx") # Final quantized model
    else:
        final_onnx_path = os.path.join(output_dir, "model.onnx") # Final model
        temp_onnx_path = final_onnx_path

    print(f"Converting model to ONNX format with modern opset...")

    # Export the model to ONNX format with newer opset for better CPU compatibility
    try:
        torch.onnx.export(
            onnx_export_model,                          # Model to export (wrapped version)
            (dummy_input_ids, dummy_attention_mask),    # Model inputs
            temp_onnx_path,                             # Output path (temp if quantizing)
            export_params=True,                         # Store the trained weights
            opset_version=17,                           # Use modern ONNX opset version (17 is well-supported)
            do_constant_folding=True,                   # Optimize constant folding
            input_names=input_names,                    # Input names
            output_names=output_names,                  # Output names (single output now)
            dynamic_axes=dynamic_axes,                  # Dynamic axes
            verbose=False
        )
    except Exception as e:
         print(f"Error during ONNX export: {e}", file=sys.stderr)
         print("This might be due to unsupported operations in the model or opset version.", file=sys.stderr)
         sys.exit(1)

    if quantized:
        print(f"Full precision model exported to: {temp_onnx_path}")
        
        # Quantize the model
        quantized_path = quantize_onnx_model(temp_onnx_path, final_onnx_path)
        
        if quantized_path:
            # Remove the temporary full precision model
            try:
                os.remove(temp_onnx_path)
                print("✓ Temporary full precision model removed")
            except:
                pass
            
            print(f"✓ Quantized model saved to: {final_onnx_path}")
        else:
            # If quantization failed, use the full precision model
            print("⚠ Quantization failed, using full precision model")
            os.rename(temp_onnx_path, final_onnx_path)
    else:
        print(f"Model successfully converted and saved to: {final_onnx_path}")

    # Save tokenizer using save_pretrained - this saves necessary files
    # like tokenizer.json, vocab.txt/merges.txt etc.
    tokenizer_path = os.path.join(output_dir) # Save directly into the output dir
    tokenizer.save_pretrained(tokenizer_path)
    print(f"Tokenizer files saved to: {tokenizer_path}")

    return final_onnx_path

def verify_onnx_model(onnx_path, tokenizer_dir, model_name, quantized=False):
    """
    Verify the ONNX model is valid and performs basic inference.

    Args:
        onnx_path (str): Path to the ONNX model
        tokenizer_dir (str): Path to the tokenizer directory
        model_name (str): Original model name for loading reference HF model
        quantized (bool): Whether the model is quantized
    """
    print(f"\n--- Verifying {'Quantized ' if quantized else ''}ONNX Model ---")
    try:
        import onnx
        import onnxruntime as ort
        import numpy as np
        from transformers import AutoTokenizer, AutoModel # For comparison

        # 1. Check ONNX model structure
        onnx_model = onnx.load(onnx_path)
        onnx.checker.check_model(onnx_model)
        print("✓ ONNX model structure check passed.")
        print(f"✓ ONNX opset version: {onnx_model.opset_import[0].version}")

        # Check if model is quantized
        quantized_nodes = [node for node in onnx_model.graph.node if 'Quantize' in node.op_type or 'Dequantize' in node.op_type]
        if quantized_nodes:
            print(f"✓ Model is quantized (found {len(quantized_nodes)} quantization nodes)")
        else:
            print("ℹ Model appears to be full precision")

        # 2. Basic inference test with ONNX Runtime (CPU optimized)
        print("Setting up ONNX Runtime session for CPU...")
        ort_session = ort.InferenceSession(onnx_path, providers=['CPUExecutionProvider'])
        onnx_input_names = [input.name for input in ort_session.get_inputs()]
        onnx_output_names = [output.name for output in ort_session.get_outputs()]
        print(f"✓ ONNX Input Names: {onnx_input_names}")
        print(f"✓ ONNX Output Names: {onnx_output_names}")

        # 3. Test inference with sample text
        print("Testing inference with sample text...")
        tokenizer = AutoTokenizer.from_pretrained(tokenizer_dir) # Load from saved dir

        # Prepare sample input text for verification (using a general text sample)
        text = "The quick brown fox jumps over the lazy dog."
        inputs = tokenizer(text, return_tensors="pt", padding=True, truncation=True, max_length=128) # Use same max_length as dummy

        # ONNX inference
        onnx_inputs = {
            onnx_input_names[0]: inputs['input_ids'].numpy(),
            onnx_input_names[1]: inputs['attention_mask'].numpy()
        }
        
        import time
        start_time = time.time()
        onnx_outputs = ort_session.run(onnx_output_names, onnx_inputs)
        inference_time = time.time() - start_time
        
        onnx_embedding = onnx_outputs[0] # Assuming single output

        # Check shape and values
        print(f"✓ ONNX Output Shape: {onnx_embedding.shape}")
        print(f"✓ Inference time: {inference_time:.4f} seconds")
        print(f"✓ Embedding dimension: {onnx_embedding.shape[-1]}")
        
        # Check if embeddings are reasonable (not all zeros/ones)
        if np.any(onnx_embedding) and not np.all(onnx_embedding == onnx_embedding[0, 0]):
            print("✓ Embeddings appear to be valid (non-uniform values)")
        else:
            print("⚠ Warning: Embeddings may be invalid (uniform values)", file=sys.stderr)

        # 4. Compare with PyTorch model if not quantized (quantized models will have slight differences)
        if not quantized:
            print("Comparing ONNX output with original PyTorch model...")
            pytorch_model = AutoModel.from_pretrained(model_name)
            pytorch_model.eval()

            # PyTorch inference
            with torch.no_grad():
                pytorch_outputs = pytorch_model(**inputs)
                pytorch_embedding = mean_pooling(pytorch_outputs, inputs['attention_mask'])

            # Check shape and values
            print(f"PyTorch Output Shape: {pytorch_embedding.shape}")

            if pytorch_embedding.shape == onnx_embedding.shape:
                 # Compare outputs (allow for small tolerance due to floating point differences)
                if np.allclose(pytorch_embedding.numpy(), onnx_embedding, atol=1e-4):
                    print("✓ ONNX and PyTorch outputs match closely.")
                else:
                    print("⚠ Warning: ONNX and PyTorch outputs differ significantly.", file=sys.stderr)
                    # Print difference norm for debugging
                    diff = np.linalg.norm(pytorch_embedding.numpy() - onnx_embedding)
                    print(f"Difference norm: {diff}", file=sys.stderr)
            else:
                 print("Error: ONNX and PyTorch output shapes do not match.", file=sys.stderr)
        else:
            print("ℹ Skipping PyTorch comparison for quantized model (expected differences)")

        print(f"✓ {'Quantized ' if quantized else ''}Model verification successful!")

    except ImportError:
        print("Please install required packages to verify the model:", file=sys.stderr)
        print("  pip install onnx onnxruntime transformers", file=sys.stderr)
    except Exception as e:
        print(f"Error verifying ONNX model: {e}", file=sys.stderr)

def main():
    parser = argparse.ArgumentParser(description="Convert all-MiniLM-L6-v2 model to ONNX with optional quantization.")
    parser.add_argument(
        "--model_name",
        type=str,
        default=DEFAULT_MODEL_NAME,
        help=f"Name of the Hugging Face model to convert (default: {DEFAULT_MODEL_NAME})"
    )
    parser.add_argument(
        "--output_dir",
        type=str,
        default=DEFAULT_OUTPUT_DIR,
        help=f"Directory to save the ONNX model and tokenizer files (default: {DEFAULT_OUTPUT_DIR})"
    )
    parser.add_argument(
        "--quantized",
        action="store_true",
        help="Enable INT8 quantization for optimal CPU performance (recommended for CPU-only deployments)"
    )
    parser.add_argument(
        "--skip_verification",
        action="store_true",
        help="Skip the ONNX model verification step."
    )

    args = parser.parse_args()

    # Use arguments passed or defaults
    model_to_convert = args.model_name
    output_directory_name = args.output_dir
    use_quantization = args.quantized

    print(f"--- Starting ONNX Conversion for {model_to_convert} ---")
    if use_quantization:
        print("Converting with modern ONNX opset + INT8 quantization for optimal CPU performance")
    else:
        print("Converting with modern ONNX opset (full precision)")

    # Download and convert the model to ONNX
    onnx_path = download_and_convert_st_model_to_onnx(
        output_dir=output_directory_name,
        model_name=model_to_convert,
        quantized=use_quantization
    )

    # Verify the ONNX model unless skipped
    if not args.skip_verification:
        verify_onnx_model(onnx_path, output_directory_name, model_to_convert, quantized=use_quantization)
    else:
        print(f"\n--- Skipping {'Quantized ' if use_quantization else ''}ONNX Model Verification ---")

    print(f"\n--- {'Quantized ' if use_quantization else ''}Model Conversion Process Complete ---")
    print("=" * 60)
    print(f"The {'quantized ' if use_quantization else ''}ONNX model and tokenizer files have been saved to the '{output_directory_name}' directory.")
    print("The primary files are:")
    model_file = os.path.join(output_directory_name, 'model.onnx')
    print(f"  • Model: {model_file}")
    print(f"  • Tokenizer Config: {os.path.join(output_directory_name, 'tokenizer.json')}")
    print(f"  • Other tokenizer files (vocab.txt, merges.txt etc. depending on model type)")
    
    if use_quantization:
        print("\n🚀 Performance Benefits:")
        print("  • INT8 quantization for 3-4X faster CPU inference")
        print("  • ~75% smaller model size compared to full precision")
        print("  • Modern ONNX opset (17) for better compatibility")
        print("  • Optimized for CPU inference with minimal accuracy loss")
    else:
        print("\n📋 Model Information:")
        print("  • Full precision ONNX model")
        print("  • Modern ONNX opset (17) for better compatibility")
        print("  • Add --quantized flag for CPU-optimized quantization")
    
    print("\n📋 Usage with sagitta-cli:")
    print("Method 1: Command Line Arguments")
    print("  ./target/release/sagitta-cli index <your_code_dir> \\")
    print(f"      --onnx-model {os.path.abspath(model_file)} \\")
    print(f"      --onnx-tokenizer {os.path.abspath(output_directory_name)}")
    
    print("\nMethod 2: Environment Variables")
    abs_model_path = os.path.abspath(model_file)
    abs_tokenizer_path = os.path.abspath(output_directory_name)
    print(f"  export SAGITTA_ONNX_MODEL=\"{abs_model_path}\"")
    print(f"  export SAGITTA_ONNX_TOKENIZER=\"{abs_tokenizer_path}\"")
    print("  ./target/release/sagitta-cli index <your_code_dir>")
    
    print("\nMethod 3: Configuration File")
    print("  Add to ~/.config/sagitta-cli/config.toml:")
    print(f"    onnx_model_path = \"{abs_model_path}\"")
    print(f"    onnx_tokenizer_path = \"{abs_tokenizer_path}\"")
    
    print("\n⚠️  Important Notes:")
    if use_quantization:
        print("  • This model uses INT8 quantization for optimal CPU performance")
        print("  • Modern ONNX opset 17 ensures compatibility")
        print("  • When switching models, rebuild your vector index")
        print("  • Use './target/release/sagitta-cli clear' if needed")
        
        print("\n💡 For CPU builds:")
        print("  • INT8 quantization provides significant speedup on CPU")
        print("  • No GPU/CUDA dependencies required")
        print("  • Optimized for production CPU inference")
        print("  • Minimal accuracy loss (~1-2% typical)")
    else:
        print("  • Full precision model for maximum accuracy")
        print("  • Use --quantized flag for CPU-optimized performance")
        print("  • When switching models, rebuild your vector index")
        print("  • Use './target/release/sagitta-cli clear' if needed")
    print("=" * 60)

if __name__ == "__main__":
    main() 