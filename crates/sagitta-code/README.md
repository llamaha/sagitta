# Sagitta Code

<!-- Do not update this file unless specifically asked to do so -->

**Sagitta Code** is an AI coding assistant built on top of the [sagitta-embed](../sagitta-embed) search engine with its own [reasoning-engine](../reasoning-engine). It provides intelligent code interaction, repository management, and conversation handling capabilities using Google's Gemini LLM.

Installation is currently a manual process, with future improvements to the install process being planned.

## Prerequisites

1. **Rust Toolchain**: Install from [rustup.rs](https://rustup.rs/)
2. **ONNX Runtime**: GPU-enabled version recommended (see [main README](../../README.md#prerequisites))
3. **Qdrant**: Vector database for semantic search
4. **Google Gemini API Key**: For LLM functionality

## Installation

1. **Clone and build**:
   ```bash
   git clone https://gitlab.com/amulvany/sagitta-search.git
   cd sagitta-search/crates/sagitta-code
   # Build with Cuda
   cargo build --release --all --features cuda
   ```

2. **Start Qdrant**:
   ```bash
   docker run -d --name qdrant_db -p 6333:6333 -p 6334:6334 \
       -v $(pwd)/qdrant_storage:/qdrant/storage:z \
       qdrant/qdrant:latest
   ```

3. **Create the embedding model**:
   ```bash
   # You will need some python libraries for this step (will add these, TODO)
   cd ../../scripts
   python convert_all_minilm_model.py  # or convert_bge_small_model.py
   ```

4. **Run the application**:
   ```bash
   ./target/release/sagitta-code
   ```

## Configuration

### Core Configuration (`~/.config/sagitta/config.toml`)
Contains sagitta-search settings shared across all Sagitta tools. See [configuration.md](../../docs/configuration.md) for detailed options.

Note: Most of this can be configured in the settings menu of the GUI.

### Sagitta Code Configuration (`~/.config/sagitta/sagitta_code_config.json`)
Contains sagitta-code specific settings:

```json
{
  "gemini": {
    "api_key": "your-gemini-api-key",
    "model": "gemini-2.5-flash-preview-05-20",
    "max_history_size": 20,
    "max_reasoning_steps": 50
  },
  "ui": {
    "dark_mode": true,
    "theme": "default",
    "window_width": 900,
    "window_height": 700
  },
  "conversation": {
    "auto_save": true,
    "max_conversations": 100,
    "auto_cleanup_days": 30
  }
}
```

### Data Storage
Following XDG Base Directory conventions:
- **Conversations**: `~/.local/share/sagitta/conversations/`
- **Repository Data**: `~/.local/share/sagitta/repositories/`

## License

This project is licensed under the MIT License - see the [LICENSE-MIT](../../LICENSE-MIT) file for details.
