import os

replacements = {
    "Aborting.": "aborting.",
    "received SIGINT (Ctrl+C). Cleaning up locks and exiting...": "received sigint (ctrl+c). cleaning up locks and exiting...",
    "Arch Linux": "arch linux",
}

def process_file(filepath):
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()

    new_content = content
    for old, new in replacements.items():
        new_content = new_content.replace(f'"{old}"', f'"{new}"')
        new_content = new_content.replace(f'"{old}', f'"{new}')

    if new_content != content:
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(new_content)
        print(f"Modified {filepath}")

for root, dirs, files in os.walk('src'):
    for file in files:
        if file.endswith('.rs'):
            process_file(os.path.join(root, file))

