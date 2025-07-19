import requests
import json

url = "http://localhost:1234/v1/chat/completions"
headers = {"Content-Type": "application/json"}
data = {
    "model": "default",
    "messages": [
        {"role": "system", "content": "You are a helpful assistant. Use <thinking> tags to show your reasoning."},
        {"role": "user", "content": "What is 2+2? Think step by step."}
    ],
    "stream": True
}

response = requests.post(url, headers=headers, json=data, stream=True)

print("=== Raw SSE Stream ===")
for line in response.iter_lines():
    if line:
        print(line.decode('utf-8'))
        if line.decode('utf-8').startswith('data: ') and line.decode('utf-8') != 'data: [DONE]':
            try:
                json_data = json.loads(line.decode('utf-8')[6:])
                if 'choices' in json_data and json_data['choices']:
                    delta = json_data['choices'][0].get('delta', {})
                    if 'content' in delta and delta['content']:
                        print(f"  -> Content: {repr(delta['content'])}")
            except:
                pass
