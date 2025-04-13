#!/usr/bin/env python
# Convert CodeBERT to ONNX format

import os
import torch
from transformers import RobertaModel, RobertaTokenizer
from pathlib import Path
import sys

def download_and_convert_codebert_to_onnx(output_dir="codebert_onnx", model_name="microsoft/codebert-base"):
    """
    Downloads CodeBERT model and converts it to ONNX format
    
    Args:
        output_dir (str): Directory to save the ONNX model
        model_name (str): Name of the CodeBERT model to download
    
    Returns:
        Path to the saved ONNX model
    """
    print(f"Downloading and loading {model_name} model...")
    
    # Create output directory if it doesn't exist
    os.makedirs(output_dir, exist_ok=True)
    
    # Load tokenizer and model
    tokenizer = RobertaTokenizer.from_pretrained(model_name)
    model = RobertaModel.from_pretrained(model_name)
    
    # Set model to evaluation mode
    model.eval()
    
    # Prepare dummy inputs for tracing
    dummy_input_ids = torch.ones(1, 512, dtype=torch.long)
    dummy_attention_mask = torch.ones(1, 512, dtype=torch.long)
    
    # Define symbolic names for inputs
    input_names = ["input_ids", "attention_mask"]
    output_names = ["last_hidden_state", "pooler_output"]
    
    # Define dynamic axes
    dynamic_axes = {
        "input_ids": {0: "batch_size", 1: "sequence_length"},
        "attention_mask": {0: "batch_size", 1: "sequence_length"},
        "last_hidden_state": {0: "batch_size", 1: "sequence_length"},
        "pooler_output": {0: "batch_size"}
    }
    
    # Define output path
    onnx_path = os.path.join(output_dir, "codebert_model.onnx")
    
    print(f"Converting model to ONNX format...")
    
    # Export the model to ONNX format
    torch.onnx.export(
        model,                                      # Model to export
        (dummy_input_ids, dummy_attention_mask),    # Model inputs
        onnx_path,                                  # Output path
        export_params=True,                         # Store the trained weights
        opset_version=14,                           # ONNX version to use
        do_constant_folding=True,                   # Optimize constant folding
        input_names=input_names,                    # Input names
        output_names=output_names,                  # Output names
        dynamic_axes=dynamic_axes,                  # Dynamic axes
        verbose=False
    )
    
    print(f"Model successfully converted and saved to: {onnx_path}")
    
    # Save tokenizer
    tokenizer_path = os.path.join(output_dir, "tokenizer")
    tokenizer.save_pretrained(tokenizer_path)
    print(f"Tokenizer saved to: {tokenizer_path}")

    # --- Explicitly create tokenizer.json using the tokenizers library ---
    try:
        from tokenizers import Tokenizer
        from tokenizers.models import BPE
        
        # Paths to the files saved by save_pretrained
        vocab_file = os.path.join(tokenizer_path, "vocab.json")
        merges_file = os.path.join(tokenizer_path, "merges.txt")
        output_tokenizer_json = os.path.join(tokenizer_path, "tokenizer.json")

        if os.path.exists(vocab_file) and os.path.exists(merges_file):
            print(f"Found vocab: {vocab_file}, merges: {merges_file}")
            # Initialize a BPE model from the vocab and merges files
            # Note: Roberta uses BPE. Adjust if using a different model type.
            # We might need to load special tokens from special_tokens_map.json 
            # and tokenizer_config.json for a more complete tokenizer, but 
            # start with just vocab/merges for compatibility with Rust.
            tokenizer_lib = Tokenizer(BPE.from_file(vocab_file, merges_file))
            
            # Set truncation (optional but good practice)
            # Check tokenizer_config.json or model defaults for appropriate length
            # Example: tokenizer_lib.enable_truncation(max_length=512)
            
            # Set padding (optional but good practice)
            # Example: tokenizer_lib.enable_padding(pad_id=1, pad_token="<pad>") # Check config for pad token/id

            # Save the combined tokenizer.json
            tokenizer_lib.save(output_tokenizer_json)
            print(f"Successfully created explicit tokenizer.json at: {output_tokenizer_json}")
        else:
            print("Warning: Could not find vocab.json and/or merges.txt to create explicit tokenizer.json", file=sys.stderr)
    except ImportError:
        print("Warning: 'tokenizers' library not found. Cannot create explicit tokenizer.json.", file=sys.stderr)
    except Exception as e:
        print(f"Error creating explicit tokenizer.json: {e}", file=sys.stderr)
    # --- End explicit creation ---

    return onnx_path

def verify_onnx_model(onnx_path):
    """
    Verify the ONNX model is valid
    
    Args:
        onnx_path (str): Path to the ONNX model
    """
    try:
        import onnx
        import onnxruntime as ort
        
        # Load and check ONNX model
        onnx_model = onnx.load(onnx_path)
        onnx.checker.check_model(onnx_model)
        print("ONNX model is valid.")
        
        # Basic inference test with ONNX Runtime
        ort_session = ort.InferenceSession(onnx_path)
        input_names = [input.name for input in ort_session.get_inputs()]
        
        # Create random input
        dummy_input_ids = torch.ones(1, 512, dtype=torch.long).numpy()
        dummy_attention_mask = torch.ones(1, 512, dtype=torch.long).numpy()
        
        # Run inference with ONNX Runtime
        inputs = {
            input_names[0]: dummy_input_ids,
            input_names[1]: dummy_attention_mask
        }
        
        outputs = ort_session.run(None, inputs)
        print("ONNX inference test successful.")
        print(f"Output shape: {outputs[0].shape}")
        
    except ImportError:
        print("Please install onnx and onnxruntime packages to verify the model:")
        print("pip install onnx onnxruntime")
    except Exception as e:
        print(f"Error verifying ONNX model: {e}")

def main():
    # Define output directory name (can be customized via args later if needed)
    output_directory_name = "codebert_onnx"
    
    # Download and convert CodeBERT to ONNX
    onnx_path = download_and_convert_codebert_to_onnx(output_dir=output_directory_name)
    
    # Verify the ONNX model
    verify_onnx_model(onnx_path)
    
    print("\nModel conversion complete.")
    print("--------------------------")
    print(f"The CodeBERT ONNX model and tokenizer have been saved to the '{output_directory_name}' directory.")
    print("To use this model with vectordb-cli:")
    print("\nMethod 1: Command Line Arguments")
    print("  Provide the paths directly during indexing:")
    print(f"    ./target/debug/vectordb-cli index <your_code_dir> \\")
    print(f"        --onnx-model {os.path.join(output_directory_name, 'codebert_model.onnx')} \\")
    print(f"        --onnx-tokenizer {os.path.join(output_directory_name, 'tokenizer')}")
    print("\nMethod 2: Environment Variables")
    print("  Set the following environment variables before running vectordb-cli:")
    # Use absolute paths for env vars based on the output directory name
    abs_model_path = os.path.abspath(os.path.join(output_directory_name, 'codebert_model.onnx'))
    abs_tokenizer_path = os.path.abspath(os.path.join(output_directory_name, 'tokenizer'))
    print(f"    export VECTORDB_ONNX_MODEL=\"{abs_model_path}\"")
    print(f"    export VECTORDB_ONNX_TOKENIZER=\"{abs_tokenizer_path}\"")
    print("  Then run indexing normally:")
    print("    ./target/debug/vectordb-cli index <your_code_dir>")
    print("\nImportant Note:")
    print("  When switching between models (e.g., from the default MiniLM to CodeBERT, or vice-versa),")
    print("  the underlying vector index needs to be rebuilt due to different embedding dimensions.")
    print("  The 'index' command will automatically detect this mismatch, clear old incompatible data,")
    print("  and create a new index. Alternatively, you can manually run './target/debug/vectordb-cli clear'")
    print("  before indexing with a different model type.")
    print("--------------------------")

if __name__ == "__main__":
    main()
