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
# Properly escape markdown for JSON
DESCRIPTION=$(cat release-notes.md | sed 's/"/\\"/g' | sed ':a;N;$!ba;s/\n/\\n/g')
RELEASE_RESPONSE=$(curl -s --request POST \
  --header "PRIVATE-TOKEN: $GITLAB_TOKEN" \
  --header "Content-Type: application/json" \
  --data '{
    "name": "'"$TAG_NAME"'",
    "tag_name": "'"$TAG_NAME"'",
    "ref": "main",
    "description": "'"$DESCRIPTION"'"
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
    FULL_URL="$GITLAB_URL$FILE_URL"
    echo "File uploaded: $FULL_URL"
    
    # Debug: Show upload response
    echo "Upload response: $UPLOAD_RESPONSE"
    
    # Link asset to release
    # URL encode the tag name for the API path
    TAG_NAME_ENCODED=$(echo "$TAG_NAME" | sed 's/\//%2F/g')
    echo "Linking to release: $TAG_NAME_ENCODED"
    
    LINK_RESPONSE=$(curl -s --request POST \
      --header "PRIVATE-TOKEN: $GITLAB_TOKEN" \
      --header "Content-Type: application/json" \
      --data '{
        "name": "'"$file"'",
        "url": "'"$FULL_URL"'",
        "link_type": "package"
      }' \
      "$GITLAB_URL/api/v4/projects/$PROJECT_ID/releases/$TAG_NAME_ENCODED/assets/links")
    
    if echo "$LINK_RESPONSE" | grep -q "error"; then
      echo "Error linking $file: $LINK_RESPONSE"
    else
      echo "Successfully linked $file to release"
    fi
  fi
done

echo "✅ Release upload complete!"
echo "View at: $GITLAB_URL/amulvany/sagitta/-/releases/$TAG_NAME"
