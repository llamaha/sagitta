#!/usr/bin/env python3
"""Test script to verify MCP tools are available in MistralRS"""

import requests
import json

# Test 1: List available models
print("=== Testing MistralRS API ===")
try:
    response = requests.get("http://localhost:1234/v1/models")
    print(f"Models endpoint status: {response.status_code}")
    if response.status_code == 200:
        print(f"Available models: {json.dumps(response.json(), indent=2)}")
except Exception as e:
    print(f"Error connecting to MistralRS: {e}")

# Test 2: Send a message that should trigger tool use
print("\n=== Testing Tool Discovery ===")
messages = [
    {
        "role": "system", 
        "content": """You are a helpful AI assistant with access to MCP tools.

When asked about your tools, you should list the actual MCP tools available to you.
MCP tools are prefixed with mcp__<server_name>__<tool_name>.

Available MCP tools include:
- mcp__sagitta-mcp-stdio__list_repositories
- mcp__sagitta-mcp-stdio__shell_execute
- mcp__sagitta-mcp-stdio__read_file
- mcp__sagitta-mcp-stdio__view_file
- mcp__sagitta-mcp-stdio__edit_file
- mcp__sagitta-mcp-stdio__search_code
- mcp__sagitta-mcp-stdio__search_file_in_repository
- mcp__sagitta-mcp-stdio__repository_map
- mcp__sagitta-mcp-stdio__targeted_view
- mcp__sagitta-mcp-stdio__sync_repository
- mcp__sagitta-mcp-stdio__add_existing_repository
- mcp__sagitta-mcp-stdio__remove_repository
- mcp__sagitta-mcp-stdio__web_search
- mcp__sagitta-mcp-stdio__streaming_shell_execution
- mcp__sagitta-mcp-stdio__todo_read
- mcp__sagitta-mcp-stdio__todo_write
- mcp__sagitta-mcp-stdio__write_file
- mcp__sagitta-mcp-stdio__multi_edit_file
- mcp__sagitta-mcp-stdio__create_directory
- mcp__sagitta-mcp-stdio__validate
- mcp__sagitta-mcp-stdio__semantic_edit

Always use the full tool name with the mcp__ prefix when invoking tools."""
    },
    {
        "role": "user", 
        "content": "Please list all repositories using the MCP tool mcp__sagitta-mcp-stdio__list_repositories"
    }
]

try:
    response = requests.post(
        "http://localhost:1234/v1/chat/completions",
        json={
            "model": "default",
            "messages": messages,
            "temperature": 0.1,
            "max_tokens": 500,
            "tools": []  # Tools should be auto-discovered via MCP
        }
    )
    print(f"Chat completion status: {response.status_code}")
    if response.status_code == 200:
        result = response.json()
        print(f"Response: {json.dumps(result, indent=2)}")
    else:
        print(f"Error: {response.text}")
except Exception as e:
    print(f"Error testing chat completion: {e}")

# Test 3: Try with explicit tool calling
print("\n=== Testing Explicit Tool Call ===")
messages2 = [
    {
        "role": "user",
        "content": "Use the tool mcp__sagitta-mcp-stdio__list_repositories to list all repositories"
    }
]

try:
    response = requests.post(
        "http://localhost:1234/v1/chat/completions",
        json={
            "model": "default", 
            "messages": messages2,
            "temperature": 0.1,
            "max_tokens": 500,
            "tool_choice": "auto"  # Try to force tool usage
        }
    )
    print(f"Explicit tool call status: {response.status_code}")
    if response.status_code == 200:
        result = response.json()
        print(f"Response: {json.dumps(result, indent=2)}")
        
        # Check if tool_calls are present
        if "choices" in result and result["choices"]:
            choice = result["choices"][0]
            if "message" in choice and "tool_calls" in choice["message"]:
                print("Tool calls detected!")
            else:
                print("No tool calls in response")
    else:
        print(f"Error: {response.text}")
except Exception as e:
    print(f"Error: {e}")