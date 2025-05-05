#!/usr/bin/env python
# Convert Sentence Transformer Code Search model to ONNX format

import os
import torch
# Use AutoModel and AutoTokenizer for flexibility
from transformers import AutoModel, AutoTokenizer
from pathlib import Path
import sys
import argparse
# Add quantization import
try:
    from onnxruntime.quantization import quantize_dynamic, QuantType
except ImportError:
    quantize_dynamic = None
    QuantType = None

# Define the model we want to convert
DEFAULT_MODEL_NAME = "flax-sentence-embeddings/st-codesearch-distilroberta-base"
DEFAULT_OUTPUT_DIR = "st_code_onnx"

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


def download_and_convert_st_model_to_onnx(output_dir=DEFAULT_OUTPUT_DIR, model_name=DEFAULT_MODEL_NAME, quantize=False, quantized_model_path=None):
    """
    Downloads a Sentence Transformer model and converts it to ONNX format
    Optionally quantizes the ONNX model.

    Args:
        output_dir (str): Directory to save the ONNX model and tokenizer
        model_name (str): Name of the Sentence Transformer model to download
        quantize (bool): Whether to quantize the ONNX model
        quantized_model_path (str): Path to save the quantized ONNX model

    Returns:
        Path to the saved ONNX model (quantized if quantize=True)
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

    # Define output path
    onnx_path = os.path.join(output_dir, "model.onnx") # Generic name "model.onnx"

    print(f"Converting model to ONNX format...")

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

    print(f"Model successfully converted and saved to: {onnx_path}")

    # Quantize if requested
    if quantize:
        if quantize_dynamic is None:
            print("onnxruntime.quantization is not installed. Please install onnxruntime and onnxruntime-tools.", file=sys.stderr)
            sys.exit(1)
        if quantized_model_path is None:
            quantized_model_path = os.path.join(output_dir, "model_quantized.onnx")
        print(f"Quantizing ONNX model and saving to: {quantized_model_path}")
        try:
            quantize_dynamic(
                onnx_path,
                quantized_model_path,
                weight_type=QuantType.QInt8
            )
            print(f"Quantized model saved to: {quantized_model_path}")
            onnx_path = quantized_model_path
        except Exception as e:
            print(f"Error during quantization: {e}", file=sys.stderr)
            sys.exit(1)

    # Save tokenizer using save_pretrained - this saves necessary files
    # like tokenizer.json, vocab.txt/merges.txt etc.
    tokenizer_path = os.path.join(output_dir) # Save directly into the output dir
    tokenizer.save_pretrained(tokenizer_path)
    print(f"Tokenizer files saved to: {tokenizer_path}")

    # --- No need for the explicit tokenizer.json creation anymore ---
    # save_pretrained usually creates tokenizer.json directly if possible.
    # If not, the individual files (vocab.json/merges.txt etc.) are there
    # for the Rust tokenizers library to load.

    return onnx_path

def verify_onnx_model(onnx_path, tokenizer_dir, model_name):
    """
    Verify the ONNX model is valid and performs basic inference.

    Args:
        onnx_path (str): Path to the ONNX model
        tokenizer_dir (str): Path to the tokenizer directory
        model_name (str): Original model name for loading reference HF model
    """
    print("\n--- Verifying ONNX Model ---")
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


        # 3. Compare ONNX output with original PyTorch model output (optional but recommended)
        print("Comparing ONNX output with original PyTorch model...")
        tokenizer = AutoTokenizer.from_pretrained(tokenizer_dir) # Load from saved dir
        pytorch_model = AutoModel.from_pretrained(model_name)
        pytorch_model.eval()

        # Prepare sample input text
        text = "def example_function(input_arg): return input_arg + 1"
        inputs = tokenizer(text, return_tensors="pt", padding=True, truncation=True, max_length=128) # Use same max_length as dummy

        # PyTorch inference
        with torch.no_grad():
            pytorch_outputs = pytorch_model(**inputs)
            pytorch_embedding = mean_pooling(pytorch_outputs, inputs['attention_mask'])

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
                print("ONNX and PyTorch outputs match closely. Verification successful!")
            else:
                print("Warning: ONNX and PyTorch outputs differ significantly.", file=sys.stderr)
                # Print difference norm for debugging
                diff = np.linalg.norm(pytorch_embedding.numpy() - onnx_embedding)
                print(f"Difference norm: {diff}", file=sys.stderr)
        else:
             print("Error: ONNX and PyTorch output shapes do not match.", file=sys.stderr)


    except ImportError:
        print("Please install 'onnx', 'onnxruntime', and 'transformers' to verify the model:", file=sys.stderr)
        print("  pip install onnx onnxruntime transformers", file=sys.stderr)
    except Exception as e:
        print(f"Error verifying ONNX model: {e}", file=sys.stderr)

def main():
    parser = argparse.ArgumentParser(description="Convert Sentence Transformer models to ONNX.")
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
    parser.add_argument(
        "--quantize",
        action="store_true",
        help="Quantize the ONNX model after export."
    )
    parser.add_argument(
        "--quantized_model_path",
        type=str,
        default=None,
        help="Path to save the quantized ONNX model (default: <output_dir>/model_quantized.onnx)"
    )

    args = parser.parse_args()

    # Use arguments passed or defaults
    model_to_convert = args.model_name
    output_directory_name = args.output_dir

    print(f"--- Starting ONNX Conversion for {model_to_convert} ---")

    # Download and convert the model to ONNX (and quantize if requested)
    onnx_path = download_and_convert_st_model_to_onnx(
        output_dir=output_directory_name,
        model_name=model_to_convert,
        quantize=args.quantize,
        quantized_model_path=args.quantized_model_path
    )

    # Verify the ONNX model unless skipped
    if not args.skip_verification:
        verify_onnx_model(onnx_path, output_directory_name, model_to_convert)
    else:
        print("\n--- Skipping ONNX Model Verification ---")

    print("\n--- Model Conversion Process Complete ---")
    print("------------------------------------------")
    print(f"The ONNX model and tokenizer files have been saved to the '{output_directory_name}' directory.")
    print("The primary files are:")
    if args.quantize:
        model_file = args.quantized_model_path or os.path.join(output_directory_name, 'model_quantized.onnx')
    else:
        model_file = os.path.join(output_directory_name, 'model.onnx')
    print(f"  - Model: {model_file}")
    print(f"  - Tokenizer Config: {os.path.join(output_directory_name, 'tokenizer.json')}")
    print(f"  - Other tokenizer files (vocab.txt, merges.txt etc. depending on model type)")
    print("\nTo use this model with vectordb-cli:")
    print("  Ensure the embedding dimension in vectordb-cli's configuration or")
    print("  detection logic matches the new model if it differs from the previous one.")
    print("\nMethod 1: Command Line Arguments")
    print("  Provide the paths directly during indexing:")
    print("    ./target/release/vectordb-cli index <your_code_dir> ")
    print(f"        --onnx-model {os.path.abspath(model_file)} ")
    print(f"        --onnx-tokenizer {os.path.abspath(output_directory_name)}")
    print("\nMethod 2: Environment Variables")
    print("  Set the following environment variables before running vectordb-cli:")
    abs_model_path = os.path.abspath(model_file)
    abs_tokenizer_path = os.path.abspath(output_directory_name)
    print(f"    export VECTORDB_ONNX_MODEL=\"{abs_model_path}\"")
    print(f"    export VECTORDB_ONNX_TOKENIZER=\"{abs_tokenizer_path}\"")
    print("  Then run indexing normally:")
    print("    ./target/release/vectordb-cli index <your_code_dir>")
    print("\nMethod 3: config.toml Example")
    print("  Add the following to your ~/.config/vectordb-cli/config.toml (use absolute paths):")
    print("\n    onnx_model_path = \"{}\"".format(abs_model_path))
    print("    onnx_tokenizer_path = \"{}\"".format(abs_tokenizer_path))
    print("\nImportant Note on Re-indexing:")
    print("  When switching embedding models, the vector index MUST be rebuilt.")
    print("  The 'index' command should automatically handle clearing incompatible data.")
    print("  Alternatively, run './target/release/vectordb-cli clear' manually first.")
    print("------------------------------------------")

if __name__ == "__main__":
    main() 