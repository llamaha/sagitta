import requests
import json
import sys

url = "http://localhost:1234/v1/chat/completions"
headers = {"Content-Type": "application/json"}

# Test 1: Thinking tags
print("=== Test 1: Thinking Tags ===")
data = {
    "model": "default",
    "messages": [
        {"role": "system", "content": "You are a helpful assistant. Use <thinking> tags to show your reasoning."},
        {"role": "user", "content": "What is 2+2? Think step by step."}
    ],
    "stream": True
}

response = requests.post(url, headers=headers, json=data, stream=True)
content_parts = []
for line in response.iter_lines():
    if line and line.decode('utf-8').startswith('data: ') and line.decode('utf-8') != 'data: [DONE]':
        try:
            json_data = json.loads(line.decode('utf-8')[6:])
            if 'choices' in json_data and json_data['choices']:
                delta = json_data['choices'][0].get('delta', {})
                if 'content' in delta and delta['content']:
                    content_parts.append(delta['content'])
        except:
            pass

full_content = ''.join(content_parts)
print(f"Full streamed content: {repr(full_content[:200])}")
if '<think>' in full_content or '<thinking>' in full_content:
    print("WARNING: Raw thinking tags found in stream\!")

# Test 2: Multiple tool calls
print("\n=== Test 2: Multiple Tool Calls ===")
data = {
    "model": "default",
    "messages": [
        {"role": "system", "content": "You are a helpful assistant that can use tools."},
        {"role": "user", "content": "What is the weather in San Francisco, New York, and London? Use the get_weather tool for each city."}
    ],
    "tools": [
        {
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get the current weather for a location",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "location": {"type": "string", "description": "The city to get weather for"}
                    },
                    "required": ["location"]
                }
            }
        }
    ],
    "stream": True
}

response = requests.post(url, headers=headers, json=data, stream=True)
tool_calls_count = 0
finish_reason = None

for line in response.iter_lines():
    if line and line.decode('utf-8').startswith('data: ') and line.decode('utf-8') != 'data: [DONE]':
        try:
            json_data = json.loads(line.decode('utf-8')[6:])
            if 'choices' in json_data and json_data['choices']:
                choice = json_data['choices'][0]
                delta = choice.get('delta', {})
                
                # Check for tool calls
                if 'tool_calls' in delta and delta['tool_calls']:
                    for tc in delta['tool_calls']:
                        if 'id' in tc:
                            tool_calls_count += 1
                            print(f"Tool call {tool_calls_count}: {tc.get('id', 'no-id')}")
                
                # Check finish reason
                if 'finish_reason' in choice and choice['finish_reason']:
                    finish_reason = choice['finish_reason']
        except Exception as e:
            print(f"Error parsing: {e}")

print(f"\nTotal tool calls found: {tool_calls_count}")
print(f"Finish reason: {finish_reason}")

if tool_calls_count < 3:
    print("WARNING: Expected at least 3 tool calls\!")
