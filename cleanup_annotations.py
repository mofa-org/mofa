import re
import os

def cleanup_tool_execute(filepath):
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()

    # Undo annotations for .execute() and tool.execute()
    # Replace `let result: Result<_, _> = (.*\.execute\(.*?\))\.await;`
    # with `let result = \1.await;`
    
    new_content = re.sub(r'let (\w+): Result<_, _> = (.*\.execute\(.*?\))\.await;', r'let \1 = \2.await;', content)

    if new_content != content:
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(new_content)
        print(f"Cleaned up {filepath}")

for root, dirs, files in os.walk('crates'):
    for file in files:
        if file.endswith('.rs'):
            cleanup_tool_execute(os.path.join(root, file))

for root, dirs, files in os.walk('tests'):
    for file in files:
        if file.endswith('.rs'):
            cleanup_tool_execute(os.path.join(root, file))
