import os
import subprocess
import re

while True:
    result = subprocess.run(['make', '-C', 'userland/xhci'], capture_output=True, text=True)
    if result.returncode == 0:
        print("Build successful!")
        break
    
    match = re.search(r'fatal error: (.*?): No existe el archivo o el directorio', result.stderr)
    if not match:
        print("Unknown error:")
        print(result.stderr)
        break
        
    header = match.group(1)
    filepath = os.path.join('userland/xhci/include', header)
    os.makedirs(os.path.dirname(filepath), exist_ok=True)
    with open(filepath, 'w') as f:
        f.write(f'#ifndef _{header.replace("/", "_").replace(".", "_").upper()}\n')
        f.write(f'#define _{header.replace("/", "_").replace(".", "_").upper()}\n\n')
        f.write('#endif\n')
    print(f"Created stub: {header}")
