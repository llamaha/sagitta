#!/usr/bin/env python
# Convert intfloat/e5-large-v2 Sentence Transformer model to ONNX format (1024-dim)

import os
import torch
from transformers import AutoModel, AutoTokenizer
from pathlib import Path
import sys
import argparse

DEFAULT_MODEL_NAME = "intfloat/e5-large-v2"
DEFAULT_OUTPUT_DIR = "e5_large_v2_onnx"

def mean_pooling(model_output, attention_mask):
    token_embeddings = model_output[0]
    input_mask_expanded = attention_mask.unsqueeze(-1).expand(token_embeddings.size()).float()
    sum_embeddings = torch.sum(token_embeddings * input_mask_expanded, 1)
    sum_mask = torch.clamp(input_mask_expanded.sum(1), min=1e-9)
    return sum_embeddings / sum_mask

class SentenceTransformerONNX(torch.nn.Module):
    def __init__(self, model):
        super().__init__()
        self.model = model

    def forward(self, input_ids, attention_mask):
        model_output = self.model(input_ids=input_ids, attention_mask=attention_mask)
        sentence_embeddings = mean_pooling(model_output, attention_mask)
        return sentence_embeddings

def download_and_convert_st_model_to_onnx(output_dir=DEFAULT_OUTPUT_DIR, model_name=DEFAULT_MODEL_NAME):
    print(f"Downloading and loading {model_name} model...")
    os.makedirs(output_dir, exist_ok=True)
    try:
        tokenizer = AutoTokenizer.from_pretrained(model_name)
        model = AutoModel.from_pretrained(model_name)
    except Exception as e:
        print(f"Error loading model/tokenizer {model_name}: {e}", file=sys.stderr)
        sys.exit(1)
    onnx_export_model = SentenceTransformerONNX(model)
    onnx_export_model.eval()
    seq_len = 128
    dummy_input_ids = torch.ones(1, seq_len, dtype=torch.long)
    dummy_attention_mask = torch.ones(1, seq_len, dtype=torch.long)
    input_names = ["input_ids", "attention_mask"]
    output_names = ["sentence_embedding"]
    dynamic_axes = {
        "input_ids": {0: "batch_size", 1: "sequence_length"},
        "attention_mask": {0: "batch_size", 1: "sequence_length"},
        "sentence_embedding": {0: "batch_size"}
    }
    onnx_path = os.path.join(output_dir, "model.onnx")
    print(f"Converting model to ONNX format...")
    try:
        torch.onnx.export(
            onnx_export_model,
            (dummy_input_ids, dummy_attention_mask),
            onnx_path,
            export_params=True,
            opset_version=14,
            do_constant_folding=True,
            input_names=input_names,
            output_names=output_names,
            dynamic_axes=dynamic_axes,
            verbose=False
        )
    except Exception as e:
        print(f"Error during ONNX export: {e}", file=sys.stderr)
        sys.exit(1)
    print(f"Model successfully converted and saved to: {onnx_path}")
    tokenizer_path = os.path.join(output_dir)
    tokenizer.save_pretrained(tokenizer_path)
    print(f"Tokenizer files saved to: {tokenizer_path}")
    return onnx_path

def verify_onnx_model(onnx_path, tokenizer_dir, model_name):
    print("\n--- Verifying ONNX Model ---")
    try:
        import onnx
        import onnxruntime as ort
        import numpy as np
        from transformers import AutoTokenizer, AutoModel
        onnx_model = onnx.load(onnx_path)
        onnx.checker.check_model(onnx_model)
        print("ONNX model structure check passed.")
        ort_session = ort.InferenceSession(onnx_path, providers=['CPUExecutionProvider'])
        onnx_input_names = [input.name for input in ort_session.get_inputs()]
        onnx_output_names = [output.name for output in ort_session.get_outputs()]
        print(f"ONNX Input Names: {onnx_input_names}")
        print(f"ONNX Output Names: {onnx_output_names}")
        print("Comparing ONNX output with original PyTorch model...")
        tokenizer = AutoTokenizer.from_pretrained(tokenizer_dir)
        pytorch_model = AutoModel.from_pretrained(model_name)
        pytorch_model.eval()
        text = "The quick brown fox jumps over the lazy dog."
        inputs = tokenizer(text, return_tensors="pt", padding=True, truncation=True, max_length=128)
        with torch.no_grad():
            pytorch_outputs = pytorch_model(**inputs)
            pytorch_embedding = mean_pooling(pytorch_outputs, inputs['attention_mask'])
        onnx_inputs = {
            onnx_input_names[0]: inputs['input_ids'].numpy(),
            onnx_input_names[1]: inputs['attention_mask'].numpy()
        }
        onnx_outputs = ort_session.run(onnx_output_names, onnx_inputs)
        onnx_embedding = onnx_outputs[0]
        print(f"PyTorch Output Shape: {pytorch_embedding.shape}")
        print(f"ONNX Output Shape: {onnx_embedding.shape}")
        if pytorch_embedding.shape == onnx_embedding.shape:
            import numpy as np
            if np.allclose(pytorch_embedding.numpy(), onnx_embedding, atol=1e-4):
                print("ONNX and PyTorch outputs match closely. Verification successful!")
            else:
                print("Warning: ONNX and PyTorch outputs differ significantly.", file=sys.stderr)
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
    parser = argparse.ArgumentParser(description="Convert intfloat/e5-large-v2 model to ONNX (1024-dim).")
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
    model_to_convert = args.model_name
    output_directory_name = args.output_dir
    print(f"--- Starting ONNX Conversion for {model_to_convert} ---")
    onnx_path = download_and_convert_st_model_to_onnx(
        output_dir=output_directory_name,
        model_name=model_to_convert
    )
    if not args.skip_verification:
        verify_onnx_model(onnx_path, output_directory_name, model_to_convert)
    else:
        print("\n--- Skipping ONNX Model Verification ---")
    print("\n--- Model Conversion Process Complete ---")
    print("------------------------------------------")
    print(f"The ONNX model and tokenizer files have been saved to the '{output_directory_name}' directory.")
    print("The primary files are:")
    model_file = os.path.join(output_directory_name, 'model.onnx')
    print(f"  - Model: {model_file}")
    print(f"  - Tokenizer Config: {os.path.join(output_directory_name, 'tokenizer.json')}")
    print(f"  - Other tokenizer files (vocab.txt, merges.txt etc. depending on model type)")
    print("\nTo use this model with vectordb-cli:")
    print("  Ensure the embedding dimension in vectordb-cli's configuration or")
    print("  detection logic matches the new model if it differs from the previous one.")
    print("\nMethod 1: Command Line Arguments")
    print("  Provide the paths directly during indexing:")

if __name__ == "__main__":
    main() 