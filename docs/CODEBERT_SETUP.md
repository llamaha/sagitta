# Using CodeBERT and Other Embedding Models

You can use other sentence-transformer models compatible with ONNX instead of the default `all-MiniLM-L6-v2`. CodeBERT is one example, specifically trained on code.

## Using CodeBERT

1.  **Generate ONNX Model & Tokenizer:**
    -   Run the provided Python script:
        ```bash
        # Ensure you have Python and necessary libraries (transformers, torch, onnx, tokenizers)
        # pip install transformers torch onnx tokenizers
        python scripts/codebert.py
        ```
    -   This will download the `microsoft/codebert-base` model, convert it to ONNX format, and save it along with its tokenizer files into the `codebert_onnx/` directory.
    -   The script will output instructions on how to use these files with `vectordb-cli`.

2.  **Configure `vectordb-cli`:** You **must** tell `vectordb-cli` where to find the CodeBERT model and tokenizer using **either** environment variables **or** command-line arguments:

    *   **Environment Variables:** (Set these in your shell or `.bashrc`/`.zshrc`)
        ```bash
        export VECTORDB_ONNX_MODEL="/path/to/your/vectordb-cli/codebert_onnx/codebert_model.onnx"
        export VECTORDB_ONNX_TOKENIZER="/path/to/your/vectordb-cli/codebert_onnx/tokenizer"
        ```
        Then run `vectordb-cli index ...` normally.

    *   **Command-Line Arguments (during `index`):**
        ```bash
        vectordb-cli index ./your/code \
          --onnx-model ./codebert_onnx/codebert_model.onnx \
          --onnx-tokenizer ./codebert_onnx/tokenizer
        ```

## MiniLM vs. CodeBERT Comparison

| Feature             | Default (all-MiniLM-L6-v2)               | CodeBERT (microsoft/codebert-base)           |
| ------------------- | ---------------------------------------- | -------------------------------------------- |
| **Primary Use**     | General semantic search                  | Semantic search focused on source code     |
| **Speed**           | Faster                                   | Slower                                       |
| **Accuracy (General)**| Good all-rounder                         | Potentially less accurate on non-code text |
| **Accuracy (Code)** | Decent                                   | Potentially higher for supported languages |
| **Language Focus**  | Broad (trained on diverse web text)      | Specific (Python, Java, JS, PHP, Ruby, Go) |
| **Dimension**       | 384                                      | 768                                          |
| **Index Size**      | Smaller                                  | Larger (due to higher dimension)           |
| **Memory Usage**    | Lower                                    | Higher                                       |
| **Setup**           | Included (via Git LFS)                   | Requires generation script (`scripts/codebert.py`) |

**Recommendation:** Start with the default MiniLM model. If you primarily work with the languages CodeBERT supports and find MiniLM's code-specific results lacking, try generating and using CodeBERT. Note that while CodeBERT is specialized for code, its performance within this tool's hybrid search algorithm (relative to MiniLM) has not been extensively tested or optimized, and may vary depending on your codebase and queries.

## Switching Models

**Important:** Different models usually produce embeddings of different dimensions (e.g., MiniLM=384, CodeBERT=768). The vector index (`hnsw_index.json`) is tied to a specific dimension.

-   When you run `vectordb-cli index` using a model with a different dimension than the one used to create the existing index, the tool will automatically detect the mismatch.
-   It will **clear the existing incompatible embeddings** from the database and **create a new vector index** compatible with the new model.
-   Alternatively, you can manually run `vectordb-cli clear` before indexing with a different model to ensure a clean state.

Failure to provide a valid model and tokenizer will result in an error. 