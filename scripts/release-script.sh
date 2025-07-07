#!/bin/bash
set -e

# Configuration
CRATE_NAME="sagitta-code"
CRATE_DIR="./crates/$CRATE_NAME"
VERSION=$(grep "^version" "$CRATE_DIR/Cargo.toml" | sed 's/version = "\(.*\)"/\1/')
RELEASE_DIR="releases/v$VERSION"
PROJECT_NAME="sagitta-code"

echo "🚀 Building $PROJECT_NAME v$VERSION releases..."

# Create release directory
mkdir -p "$RELEASE_DIR"

# Function to build and package a variant
build_variant() {
    local variant=$1
    local features=$2
    local build_cmd=$3
    
    echo "📦 Building $variant variant..."
    
    # Build the variant
    if [ -n "$features" ]; then
        echo "Running: cargo build --release --all --features $features"
        cargo build --release --all --features "$features"
    else
        echo "Running: cargo build --release --all"
        cargo build --release --all
    fi
    
    # Find the binary
    local binary_path="target/release/$CRATE_NAME"
    if [ ! -f "$binary_path" ]; then
        echo "❌ Binary not found at $binary_path"
        return 1
    fi
    
    # Create variant-specific directory
    local variant_dir="$RELEASE_DIR/$variant"
    mkdir -p "$variant_dir"
    
    # Copy binary with variant-specific name
    local binary_name="${PROJECT_NAME}-v${VERSION}-${variant}"
    cp "$binary_path" "$variant_dir/$binary_name"
    
    # Create tarball
    cd "$variant_dir"
    tar -czf "../${binary_name}.tar.gz" "$binary_name"
    cd - > /dev/null
    
    # Create checksum
    cd "$RELEASE_DIR"
    sha256sum "${binary_name}.tar.gz" >> "checksums.txt"
    cd - > /dev/null
    
    echo "✅ $variant build complete: $RELEASE_DIR/${binary_name}.tar.gz"
}

# Clean previous builds
echo "🧹 Cleaning previous builds..."
cargo clean

# Build all variants
echo "🔨 Building all variants..."

build_variant "linux-cpu" "" "cargo build --release --all"
build_variant "linux-cuda" "cuda" "cargo build --release --all --features cuda"
build_variant "linux-rocm" "rocm" "cargo build --release --all --features rocm"

# Generate release notes template
cat > "$RELEASE_DIR/release-notes.md" << EOF
# $PROJECT_NAME v$VERSION

## What's Changed

<!-- Add your changelog entries here -->

## Downloads

### Linux CPU
- \`${PROJECT_NAME}-v${VERSION}-linux-cpu.tar.gz\` - Standard Linux build

### Linux CUDA
- \`${PROJECT_NAME}-v${VERSION}-linux-cuda.tar.gz\` - Linux build with CUDA support

### Linux ROCm
- \`${PROJECT_NAME}-v${VERSION}-linux-rocm.tar.gz\` - Linux build with ROCm support

## Installation

1. Download the appropriate variant for your system
2. Extract: \`tar -xzf ${PROJECT_NAME}-v${VERSION}-<variant>.tar.gz\`
3. Make executable: \`chmod +x ${PROJECT_NAME}-v${VERSION}-<variant>\`
4. Optionally move to PATH: \`sudo mv ${PROJECT_NAME}-v${VERSION}-<variant> /usr/local/bin/${PROJECT_NAME}\`

## Checksums

\`\`\`
$(cat "$RELEASE_DIR/checksums.txt")
\`\`\`

**Full Changelog**: https://gitlab.com/youruser/yourproject/-/compare/v$(echo $VERSION | awk -F. '{print $1"."$2"."$3-1}')...v$VERSION
EOF

# Create upload script
cat > "$RELEASE_DIR/upload-to-gitlab.sh" << 'EOF'
#!/bin/bash
set -e

# Configuration - UPDATE THESE VALUES
PROJECT_ID=${PROJECT_ID}  # Get from GitLab project settings
GITLAB_TOKEN=${GITLAB_TOKEN}  # Personal access token with API scope
GITLAB_URL="https://gitlab.com"  # Change if using self-hosted GitLab

VERSION=$(basename $(pwd) | sed 's/v//')
TAG_NAME="v$VERSION"

echo "🚀 Uploading release v$VERSION to GitLab..."

# Create the release
echo "📝 Creating release..."
RELEASE_RESPONSE=$(curl -s --request POST \
  --header "PRIVATE-TOKEN: $GITLAB_TOKEN" \
  --header "Content-Type: application/json" \
  --data '{
    "name": "'"$TAG_NAME"'",
    "tag_name": "'"$TAG_NAME"'",
    "description": "'"$(cat release-notes.md | sed 's/"/\\"/g' | tr '\n' ' ')"'"
  }' \
  "$GITLAB_URL/api/v4/projects/$PROJECT_ID/releases")

echo "Release created: $RELEASE_RESPONSE"

# Upload assets
echo "📦 Uploading assets..."
for file in *.tar.gz; do
  if [ -f "$file" ]; then
    echo "Uploading $file..."
    
    # Upload file
    UPLOAD_RESPONSE=$(curl -s --request POST \
      --header "PRIVATE-TOKEN: $GITLAB_TOKEN" \
      --form "file=@$file" \
      "$GITLAB_URL/api/v4/projects/$PROJECT_ID/uploads")
    
    # Extract URL from response
    FILE_URL=$(echo "$UPLOAD_RESPONSE" | grep -o '"url":"[^"]*"' | sed 's/"url":"\([^"]*\)"/\1/')
    echo "File uploaded: $GITLAB_URL$FILE_URL"
    
    # Link asset to release
    curl -s --request POST \
      --header "PRIVATE-TOKEN: $GITLAB_TOKEN" \
      --header "Content-Type: application/json" \
      --data '{
        "name": "'"$file"'",
        "url": "'"$GITLAB_URL$FILE_URL"'",
        "link_type": "package"
      }' \
      "$GITLAB_URL/api/v4/projects/$PROJECT_ID/releases/$TAG_NAME/links"
  fi
done

echo "✅ Release upload complete!"
echo "View at: $GITLAB_URL/youruser/yourproject/-/releases/$TAG_NAME"
EOF

chmod +x "$RELEASE_DIR/upload-to-gitlab.sh"

# Summary
echo ""
echo "🎉 Release build complete!"
echo ""
echo "📁 Release files created in: $RELEASE_DIR"
echo "📋 Files created:"
ls -la "$RELEASE_DIR"
echo ""
echo "📝 Next steps:"
echo "1. Review the release notes: $RELEASE_DIR/release-notes.md"
echo "2. Create a git tag: git tag v$VERSION && git push origin v$VERSION"
echo "3. Upload to GitLab:"
echo "   - Manual: Go to GitLab → Releases → New Release"
echo "   - Automated: Configure and run $RELEASE_DIR/upload-to-gitlab.sh"
echo ""
echo "💡 Tip: Add this script to your PATH for easy access!"
