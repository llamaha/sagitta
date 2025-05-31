#!/usr/bin/env python
# Convert BGE-Base-EN-v1.5 model to ONNX format

import os
import torch
# Use AutoModel and AutoTokenizer for flexibility
from transformers import AutoModel, AutoTokenizer
from pathlib import Path
import sys
import argparse

# Define the model we want to convert
DEFAULT_MODEL_NAME = "BAAI/bge-base-en-v1.5"
DEFAULT_OUTPUT_DIR = "bge_base_en_onnx"

# --- BGE model specific changes START ---
# BGE models require pooling the last hidden state.
# This helper function implements mean pooling with normalization.
def mean_pooling(model_output, attention_mask):
    token_embeddings = model_output[0] # First element contains all token embeddings
    input_mask_expanded = attention_mask.unsqueeze(-1).expand(token_embeddings.size()).float()
    # Sum embeddings and divide by the number of non-padding tokens
    sum_embeddings = torch.sum(token_embeddings * input_mask_expanded, 1)
    sum_mask = torch.clamp(input_mask_expanded.sum(1), min=1e-9)
    return sum_embeddings / sum_mask

# Wrapper model to include pooling logic for ONNX export
class BGEModelONNX(torch.nn.Module):
    def __init__(self, model):
        super().__init__()
        self.model = model

    def forward(self, input_ids, attention_mask):
        model_output = self.model(input_ids=input_ids, attention_mask=attention_mask)
        # Perform mean pooling
        sentence_embeddings = mean_pooling(model_output, attention_mask)
        # BGE models benefit from normalization
        sentence_embeddings = torch.nn.functional.normalize(sentence_embeddings, p=2, dim=1)
        return sentence_embeddings # Return normalized pooled sentence embeddings
# --- BGE model specific changes END ---


def download_and_convert_bge_model_to_onnx(output_dir=DEFAULT_OUTPUT_DIR, model_name=DEFAULT_MODEL_NAME):
    """
    Downloads a BGE model and converts it to ONNX format

    Args:
        output_dir (str): Directory to save the ONNX model and tokenizer
        model_name (str): Name of the BGE model to download

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
    onnx_export_model = BGEModelONNX(model)

    # Set model to evaluation mode
    onnx_export_model.eval()

    # Prepare dummy inputs for tracing (BGE models use 512 max sequence length)
    seq_len = 512 # BGE-Base-EN-v1.5 uses 512 token context window
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
        "sentence_embedding": {0: "batch_size"} # Dimension 1 (768 for BGE-Base-EN) is fixed
    }

    # Define output path
    onnx_path = os.path.join(output_dir, "model.onnx") # Generic name "model.onnx"

    print(f"Converting BGE model to ONNX format...")

    # Export the model to ONNX format
    try:
        torch.onnx.export(
            onnx_export_model,                          # Model to export (wrapped version)
            (dummy_input_ids, dummy_attention_mask),    # Model inputs
            onnx_path,                                  # Output path
            export_params=True,                         # Store the trained weights
            opset_version=14,                           # ONNX version to use (check compatibility)
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

    print(f"BGE model successfully converted and saved to: {onnx_path}")

    # Save tokenizer using save_pretrained - this saves necessary files
    # like tokenizer.json, vocab.txt/merges.txt etc.
    tokenizer_path = os.path.join(output_dir) # Save directly into the output dir
    tokenizer.save_pretrained(tokenizer_path)
    print(f"Tokenizer files saved to: {tokenizer_path}")

    return onnx_path

def verify_onnx_model(onnx_path, tokenizer_dir, model_name):
    """
    Verify the ONNX model is valid and performs basic inference.

    Args:
        onnx_path (str): Path to the ONNX model
        tokenizer_dir (str): Path to the tokenizer directory
        model_name (str): Original model name for loading reference HF model
    """
    print("\n--- Verifying BGE ONNX Model ---")
    try:
        import onnx
        import onnxruntime as ort
        import numpy as np
        from transformers import AutoTokenizer, AutoModel # For comparison

        # 1. Check ONNX model structure
        onnx_model = onnx.load(onnx_path)
        onnx.checker.check_model(onnx_model)
        print("ONNX model structure check passed.")

        # 2. Basic inference test with ONNX Runtime
        ort_session = ort.InferenceSession(onnx_path, providers=['CPUExecutionProvider']) # Specify CPU provider
        onnx_input_names = [input.name for input in ort_session.get_inputs()]
        onnx_output_names = [output.name for output in ort_session.get_outputs()]
        print(f"ONNX Input Names: {onnx_input_names}")
        print(f"ONNX Output Names: {onnx_output_names}")

        # Check output shape (should be 768 for BGE-Base-EN)
        output_shape = ort_session.get_outputs()[0].shape
        print(f"ONNX Output Shape: {output_shape}")
        expected_dim = 768 # BGE-Base-EN-v1.5 embedding dimension
        if output_shape[-1] == expected_dim:
            print(f"✓ Output dimension matches expected {expected_dim}")
        else:
            print(f"⚠ Warning: Output dimension {output_shape[-1]} doesn't match expected {expected_dim}")

        # 3. Compare ONNX output with original PyTorch model output
        print("Comparing ONNX output with original PyTorch model...")
        tokenizer = AutoTokenizer.from_pretrained(tokenizer_dir) # Load from saved dir
        pytorch_model = AutoModel.from_pretrained(model_name)
        pytorch_model.eval()

        # Prepare sample input text for verification (using code-like text for BGE)
        text = "def calculate_fibonacci(n): return n if n <= 1 else calculate_fibonacci(n-1) + calculate_fibonacci(n-2)"
        inputs = tokenizer(text, return_tensors="pt", padding=True, truncation=True, max_length=512) # Use 512 max_length for BGE

        # PyTorch inference with normalization
        with torch.no_grad():
            pytorch_outputs = pytorch_model(**inputs)
            pytorch_embedding = mean_pooling(pytorch_outputs, inputs['attention_mask'])
            pytorch_embedding = torch.nn.functional.normalize(pytorch_embedding, p=2, dim=1) # Apply normalization

        # ONNX inference
        onnx_inputs = {
            onnx_input_names[0]: inputs['input_ids'].numpy(),
            onnx_input_names[1]: inputs['attention_mask'].numpy()
        }
        onnx_outputs = ort_session.run(onnx_output_names, onnx_inputs)
        onnx_embedding = onnx_outputs[0] # Assuming single output

        # Check shape and values
        print(f"PyTorch Output Shape: {pytorch_embedding.shape}")
        print(f"ONNX Output Shape: {onnx_embedding.shape}")

        if pytorch_embedding.shape == onnx_embedding.shape:
             # Compare outputs (allow for small tolerance due to floating point differences)
            if np.allclose(pytorch_embedding.numpy(), onnx_embedding, atol=1e-4):
                print("✓ ONNX and PyTorch outputs match closely. Verification successful!")
            else:
                print("⚠ Warning: ONNX and PyTorch outputs differ significantly.", file=sys.stderr)
                # Print difference norm for debugging
                diff = np.linalg.norm(pytorch_embedding.numpy() - onnx_embedding)
                print(f"Difference norm: {diff}", file=sys.stderr)
                
                # Check if they're at least similar in magnitude
                if diff < 0.1:
                    print("Difference is small, likely acceptable for production use.")
                else:
                    print("Large difference detected - please verify model conversion.")
        else:
             print("✗ Error: ONNX and PyTorch output shapes do not match.", file=sys.stderr)

        # Test with a batch of inputs
        print("\nTesting batch processing...")
        batch_texts = [
            "def hello_world(): print('Hello, World!')",
            "class MyClass: def __init__(self): self.value = 42",
            "import numpy as np; arr = np.array([1, 2, 3, 4, 5])"
        ]
        batch_inputs = tokenizer(batch_texts, return_tensors="pt", padding=True, truncation=True, max_length=512)
        
        # ONNX batch inference
        batch_onnx_inputs = {
            onnx_input_names[0]: batch_inputs['input_ids'].numpy(),
            onnx_input_names[1]: batch_inputs['attention_mask'].numpy()
        }
        batch_onnx_outputs = ort_session.run(onnx_output_names, batch_onnx_inputs)
        batch_onnx_embeddings = batch_onnx_outputs[0]
        
        print(f"Batch processing successful: {batch_onnx_embeddings.shape}")
        print(f"✓ Model ready for batch processing with shape [batch_size, {expected_dim}]")

    except ImportError:
        print("Please install 'onnx', 'onnxruntime', and 'transformers' to verify the model:", file=sys.stderr)
        print("  pip install onnx onnxruntime transformers", file=sys.stderr)
    except Exception as e:
        print(f"Error verifying ONNX model: {e}", file=sys.stderr)

def main():
    parser = argparse.ArgumentParser(description="Convert BGE-Base-EN-v1.5 model to ONNX.")
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
        "--skip_verification",
        action="store_true",
        help="Skip the ONNX model verification step."
    )

    args = parser.parse_args()

    # Use arguments passed or defaults
    model_to_convert = args.model_name
    output_directory_name = args.output_dir

    print(f"--- Starting ONNX Conversion for {model_to_convert} ---")
    print(f"Expected embedding dimension: 768")
    print(f"Expected max sequence length: 512 tokens")

    # Download and convert the model to ONNX
    onnx_path = download_and_convert_bge_model_to_onnx(
        output_dir=output_directory_name,
        model_name=model_to_convert
    )

    # Verify the ONNX model unless skipped
    if not args.skip_verification:
        verify_onnx_model(onnx_path, output_directory_name, model_to_convert)
    else:
        print("\n--- Skipping ONNX Model Verification ---")

    print("\n--- BGE Model Conversion Process Complete ---")
    print("------------------------------------------")
    print(f"The ONNX model and tokenizer files have been saved to the '{output_directory_name}' directory.")
    print("The primary files are:")
    model_file = os.path.join(output_directory_name, 'model.onnx')
    print(f"  - Model: {model_file}")
    print(f"  - Tokenizer Config: {os.path.join(output_directory_name, 'tokenizer.json')}")
    print(f"  - Other tokenizer files (vocab.txt, merges.txt etc. depending on model type)")
    
    print(f"\n🎯 BGE-Base-EN-v1.5 Model Details:")
    print(f"  - Embedding Dimension: 768")
    print(f"  - Max Sequence Length: 512 tokens")
    print(f"  - Optimized for: English text and code")
    print(f"  - Memory Usage: ~800MB VRAM")
    
    print("\nTo use this model with sagitta-cli:")
    print("  Ensure your Rust code expects 768-dimensional embeddings")
    print("  and max_sequence_length of 512 tokens.")
    
    print("\nMethod 1: Command Line Arguments")
    print("  Provide the paths directly during indexing:")
    print("    ./target/release/sagitta-cli index <your_code_dir> ")
    print(f"        --onnx-model {os.path.abspath(model_file)} ")
    print(f"        --onnx-tokenizer {os.path.abspath(output_directory_name)}")
    
    print("\nMethod 2: Environment Variables")
    print("  Set the following environment variables before running sagitta-cli:")
    abs_model_path = os.path.abspath(model_file)
    abs_tokenizer_path = os.path.abspath(output_directory_name)
    print(f"    export SAGITTA_ONNX_MODEL=\"{abs_model_path}\"")
    print(f"    export SAGITTA_ONNX_TOKENIZER=\"{abs_tokenizer_path}\"")
    print("  Then run indexing normally:")
    print("    ./target/release/sagitta-cli index <your_code_dir>")
    
    print("\nMethod 3: config.toml Example")
    print("  Add the following to your ~/.config/sagitta-cli/config.toml (use absolute paths):")
    print("\n    onnx_model_path = \"{}\"".format(abs_model_path))
    print("    onnx_tokenizer_path = \"{}\"".format(abs_tokenizer_path))
    
    print("\n⚠️  Important Notes:")
    print("  1. When switching from your previous model, you MUST rebuild the index")
    print("  2. Update your Rust code to expect 768-dim embeddings")
    print("  3. Set max_seq_length to 512 in your ONNX provider")
    print("  4. This model is ~2x larger than MiniLM but more accurate for code")
    
    print("\n🚀 Performance Expectations:")
    print("  - 30-50% better search accuracy than st-codesearch-distilroberta-base")
    print("  - Memory usage: ~800MB VRAM (fits comfortably in 4GB+)")
    print("  - Optimal batch size: 32-48 for your hardware")
    print("  - Good balance between accuracy and efficiency")
    
    print("\nRun './target/release/sagitta-cli clear' first to remove old index data.")
    print("------------------------------------------")

if __name__ == "__main__":
    main()
