#!/usr/bin/env python
# Convert CodeBERT to ONNX format

import os
import torch
from transformers import RobertaModel, RobertaTokenizer
from pathlib import Path

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
    # Download and convert CodeBERT to ONNX
    onnx_path = download_and_convert_codebert_to_onnx()
    
    # Verify the ONNX model
    verify_onnx_model(onnx_path)
    
    print("\nModel conversion complete. Usage example:")
    print("---------------------------------------------------")
    print("import onnxruntime as ort")
    print("import numpy as np")
    print("from transformers import RobertaTokenizer")
    print()
    print("# Load tokenizer and ONNX runtime session")
    print("tokenizer = RobertaTokenizer.from_pretrained('codebert_onnx/tokenizer')")
    print("session = ort.InferenceSession('codebert_onnx/codebert_model.onnx')")
    print()
    print("# Tokenize input")
    print("code = 'def hello_world():\\n    print(\"Hello, World!\")\\n'")
    print("inputs = tokenizer(code, return_tensors='np')")
    print()
    print("# Run inference")
    print("outputs = session.run(None, {")
    print("    'input_ids': inputs['input_ids'],")
    print("    'attention_mask': inputs['attention_mask']")
    print("})")
    print("---------------------------------------------------")

if __name__ == "__main__":
    main()
