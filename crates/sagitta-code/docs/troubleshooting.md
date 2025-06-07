# Troubleshooting Guide

This guide helps you resolve common issues with Sagitta Code.

## Docker Issues

### Docker Not Installed

**Symptoms:**
- Error: "Docker is required for secure containerized execution but is not installed"
- Shell execution fails immediately
- Test execution fails with Docker error

**Solution:**
Install Docker for your platform following the [installation guide](../README.md#docker-installation).

**Quick verification:**
```bash
docker --version
docker run hello-world
```

### Docker Daemon Not Running

**Symptoms:**
- Error: "Cannot connect to the Docker daemon"
- Docker commands hang or fail
- `docker info` fails

**Solutions:**

#### Linux
```bash
# Check Docker status
sudo systemctl status docker

# Start Docker service
sudo systemctl start docker

# Enable auto-start
sudo systemctl enable docker

# Add user to docker group (logout/login required)
sudo usermod -aG docker $USER
```

#### Windows
1. Start Docker Desktop from the Start menu
2. Check system tray for Docker icon
3. If Docker Desktop won't start:
   - Restart as Administrator
   - Enable WSL 2 or Hyper-V in BIOS
   - Check Windows features (Hyper-V, Containers)

#### macOS
1. Start Docker Desktop from Applications
2. Check menu bar for Docker icon
3. If Docker Desktop won't start:
   - Check System Preferences > Security & Privacy
   - Allow Docker in Privacy settings if prompted
   - Restart Docker Desktop

### Shell/Test Execution Timeouts

**Symptoms:**
- Commands timeout after 120 seconds
- "Docker image pull taking longer than expected"
- First execution of a language takes a long time

**Causes:**
- Docker images need to be downloaded on first use
- Slow network connection
- Large container images

**Solutions:**

1. **Pre-pull common images:**
   ```bash
   docker pull megabytelabs/devcontainer:latest
   docker pull python:3.11
   docker pull rust:1.75
   docker pull node:20
   docker pull golang:1.21
   ```

2. **Check network connectivity:**
   ```bash
   # Test Docker Hub connection
   docker pull hello-world
   ```

3. **Monitor Docker image pulls:**
   ```bash
   # Watch Docker activity
   docker images
   docker ps -a
   ```

4. **Increase timeout if needed:**
   Edit `crates/sagitta-code/src/reasoning/config.rs` and increase `default_tool_timeout` beyond 120 seconds for very slow connections.

### Docker Permission Issues

**Symptoms:**
- "Permission denied" when running Docker commands
- Need to use `sudo` for Docker commands

**Solution (Linux only):**
```bash
# Add user to docker group
sudo usermod -aG docker $USER

# Apply group changes (logout/login alternative)
newgrp docker

# Test without sudo
docker run hello-world
```

## LLM and API Issues

### Gemini API Errors

**Common errors and solutions:**

1. **Invalid API Key:**
   ```
   Error: 401 Unauthorized
   ```
   - Verify API key in `~/.config/sagitta/sagitta_code_config.json`
   - Generate new key at [Google AI Studio](https://makersuite.google.com/app/apikey)

2. **Rate Limit Exceeded:**
   ```
   Error: 429 Too Many Requests
   ```
   - Wait before retrying
   - Check your quota in Google Cloud Console
   - Consider upgrading your plan

3. **Model Not Found:**
   ```
   Error: 404 Model not found
   ```
   - Verify model name in config
   - Use: `gemini-2.5-flash-preview-05-20` (recommended)

### Network Connectivity Issues

**Symptoms:**
- API calls timeout
- Web search fails
- Repository cloning fails

**Solutions:**
1. Check internet connection
2. Test specific endpoints:
   ```bash
   curl https://generativelanguage.googleapis.com/v1beta/models
   ```
3. Check firewall/proxy settings
4. Verify DNS resolution

## Repository and Indexing Issues

### Repository Cloning Fails

**Common causes:**
- Git not installed
- SSH key not configured
- Network issues
- Repository permissions

**Solutions:**
1. **Install Git:**
   ```bash
   # Check Git installation
   git --version
   
   # Install if missing
   sudo apt install git  # Ubuntu/Debian
   brew install git      # macOS
   ```

2. **Configure Git:**
   ```bash
   git config --global user.name "Your Name"
   git config --global user.email "your.email@example.com"
   ```

3. **SSH Key Setup:**
   ```bash
   # Generate SSH key
   ssh-keygen -t ed25519 -C "your.email@example.com"
   
   # Add to SSH agent
   ssh-add ~/.ssh/id_ed25519
   
   # Copy public key to clipboard
   cat ~/.ssh/id_ed25519.pub
   ```

### Indexing Failures

**Symptoms:**
- Repository status shows "Failed"
- Missing search results
- Partial indexing

**Solutions:**
1. **Check file permissions:**
   ```bash
   # Ensure read access to repository
   ls -la /path/to/repository
   ```

2. **Monitor disk space:**
   ```bash
   df -h ~/.local/share/sagitta/
   ```

3. **Clear and rebuild index:**
   - Remove repository from Sagitta Code
   - Re-add repository
   - Wait for complete indexing

### Slow Search Performance

**Causes:**
- Large repositories
- Insufficient RAM
- Slow disk I/O

**Solutions:**
1. **Optimize Qdrant settings** in `~/.config/sagitta/config.toml`
2. **Increase memory allocation:**
   ```toml
   [performance]
   max_concurrent_uploads = 4
   batch_size = 50
   ```
3. **Use SSD storage** for better I/O performance

## ONNX Runtime Issues

### Library Not Found

**Symptoms:**
- "ONNX Runtime library not found"
- Embedding generation fails
- Missing ML model support

**Solutions:**

1. **Linux:**
   ```bash
   # Download ONNX Runtime
   wget https://github.com/microsoft/onnxruntime/releases/download/v1.20.0/onnxruntime-linux-x64-1.20.0.tgz
   tar -xzf onnxruntime-linux-x64-1.20.0.tgz
   
   # Set library path
   export LD_LIBRARY_PATH=$HOME/onnxruntime-linux-x64-1.20.0/lib:$LD_LIBRARY_PATH
   
   # Make permanent
   echo 'export LD_LIBRARY_PATH=$HOME/onnxruntime-linux-x64-1.20.0/lib:$LD_LIBRARY_PATH' >> ~/.bashrc
   ```

2. **Windows:**
   - Download from [ONNX Runtime releases](https://github.com/microsoft/onnxruntime/releases)
   - Extract and add to PATH
   - Restart terminal

3. **macOS:**
   ```bash
   # Using Homebrew
   brew install onnxruntime
   
   # Or download manually and set DYLD_LIBRARY_PATH
   ```

### GPU Support Issues

**For CUDA support:**
1. Install NVIDIA drivers
2. Install CUDA toolkit
3. Download GPU-enabled ONNX Runtime
4. Verify with: `nvidia-smi`

## Qdrant Vector Database Issues

### Connection Failed

**Symptoms:**
- "Failed to connect to Qdrant"
- Search functionality disabled
- Repository indexing fails

**Solutions:**

1. **Start Qdrant container:**
   ```bash
   docker run -d --name qdrant_db -p 6333:6333 -p 6334:6334 \
       -v $(pwd)/qdrant_storage:/qdrant/storage:z \
       qdrant/qdrant:latest
   ```

2. **Check Qdrant status:**
   ```bash
   curl http://localhost:6333/health
   ```

3. **Restart Qdrant:**
   ```bash
   docker restart qdrant_db
   ```

4. **Check port conflicts:**
   ```bash
   netstat -tlnp | grep 6333
   ```

### Storage Issues

**Symptoms:**
- Qdrant container won't start
- "No space left on device"
- Corrupted collections

**Solutions:**
1. **Clean up old data:**
   ```bash
   docker volume prune
   rm -rf ./qdrant_storage/*
   ```

2. **Check disk space:**
   ```bash
   df -h
   ```

3. **Restart with fresh storage:**
   ```bash
   docker stop qdrant_db
   docker rm qdrant_db
   # Start with fresh container (command above)
   ```

## Performance Issues

### High Memory Usage

**Symptoms:**
- System becomes slow
- Out of memory errors
- Sagitta Code crashes

**Solutions:**
1. **Reduce batch sizes** in config
2. **Limit concurrent operations**
3. **Close unused repositories**
4. **Restart application periodically**

### Slow Response Times

**Causes:**
- Large repositories
- Network latency
- Resource contention

**Solutions:**
1. **Enable GPU acceleration** for embeddings
2. **Use faster storage** (SSD)
3. **Increase system RAM**
4. **Optimize repository selection**

## Configuration Issues

### Config File Not Found

**Symptoms:**
- "Configuration file not found"
- Default settings used
- Settings don't persist

**Solutions:**
1. **Create config directory:**
   ```bash
   mkdir -p ~/.config/sagitta
   ```

2. **Copy example config:**
   ```bash
   # From sagitta-code directory
   cp config/examples/sagitta_code_config.json ~/.config/sagitta/
   ```

3. **Set proper permissions:**
   ```bash
   chmod 600 ~/.config/sagitta/sagitta_code_config.json
   ```

### Config Migration Issues

**Symptoms:**
- Old settings not migrated
- Duplicate configurations
- Config conflicts

**Solutions:**
1. **Manual migration:**
   ```bash
   # Backup old config
   cp ~/.config/sagitta_code/sagitta_code_config.json ~/backup.json
   
   # Copy to new location
   cp ~/backup.json ~/.config/sagitta/sagitta_code_config.json
   ```

2. **Clean up old configs:**
   ```bash
   rm -rf ~/.config/sagitta_code
   ```

## Testing and Verification

### Verify Your Installation

Run these commands to test your setup:

```bash
# Test Docker
docker --version && docker run hello-world

# Test Qdrant
curl http://localhost:6333/health

# Test shell execution
cd crates/sagitta-code
cargo run --bin test_shell_execution

# Run unit tests
cargo test

# Test with debug logging
RUST_LOG=debug cargo run
```

### Debug Mode

Enable comprehensive debugging:

```bash
# Maximum debug output
RUST_LOG=trace cargo run

# Component-specific debugging
RUST_LOG=sagitta_code::tools::shell_execution=debug cargo run

# Log to file
RUST_LOG=debug cargo run 2>&1 | tee debug.log
```

## Getting Help

If you're still experiencing issues:

1. **Check existing issues** on GitHub
2. **Create a detailed issue report** including:
   - Operating system and version
   - Docker version
   - Error messages
   - Steps to reproduce
   - Debug logs
3. **Ask in discussions** for usage questions
4. **Review documentation** for additional guidance

## Common Error Messages

### "Docker is required for secure containerized execution but is not installed"
- **Solution**: Install Docker following the [installation guide](../README.md#docker-installation)

### "Tool orchestration timed out after 120 seconds"
- **Solution**: Large Docker images need more time on first use. Wait or pre-pull images.

### "Cannot connect to the Docker daemon"
- **Solution**: Start Docker service or Docker Desktop application

### "ONNX Runtime library not found"
- **Solution**: Install ONNX Runtime and set library path

### "Failed to connect to Qdrant at localhost:6333"
- **Solution**: Start Qdrant container using Docker

### "Gemini API key is invalid"
- **Solution**: Check API key in configuration file

---

This troubleshooting guide covers the most common issues. For additional help, please refer to the main documentation or open an issue on GitHub. 