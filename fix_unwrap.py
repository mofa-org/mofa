import os

def fix_file(filepath):
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()

    new_content = content.replace('.await.unwrap()', '.await.expect("failed")')

    if new_content != content:
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(new_content)
        print(f"Fixed {filepath}")

for root, dirs, files in os.walk('crates'):
    for file in files:
        if file.endswith('.rs'):
            fix_file(os.path.join(root, file))

for root, dirs, files in os.walk('tests'):
    for file in files:
        if file.endswith('.rs'):
            fix_file(os.path.join(root, file))
