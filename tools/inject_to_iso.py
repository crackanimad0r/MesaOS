#!/usr/bin/env python3
"""
inject_to_iso.py - Generador de initrd binario para MesaOS
Toma una lista de archivos (desde JSON) y genera output/initrd.bin

Uso desde bash:
  python3 tools/inject_to_iso.py < archivos.json
"""
import os
import sys
import json
import struct


def build_initrd(params):
    """Construye el initrd a partir de los parámetros."""
    files = params.get('files', [])
    inject_dir = params.get('inject_dir', '')
    use_default = params.get('use_default', False)
    initrd_path = params.get('initrd_path', 'output/initrd.bin')

    if not files:
        os.makedirs(os.path.dirname(initrd_path), exist_ok=True)
        with open(initrd_path, 'wb') as f:
            f.write(struct.pack('<I', 0))
        return {'ok': True, 'count': 0, 'total_bytes': 4}

    entries = []

    for src_path in files:
        if not os.path.isfile(src_path):
            print(f"  [WARN] Saltando: {src_path} (no es archivo regular)", file=sys.stderr)
            continue

        if use_default and inject_dir:
            rel = os.path.relpath(src_path, inject_dir)
            dest_path = rel
            dest_name = os.path.basename(rel)
        else:
            dest_name = os.path.basename(src_path)
            dest_path = dest_name

        with open(src_path, 'rb') as fh:
            data = fh.read()

        name_bytes = dest_name.encode('utf-8')
        path_bytes = dest_path.encode('utf-8')

        entries.append({
            'name': name_bytes,
            'path': path_bytes,
            'data': data,
        })
        print(f"  [OK] {dest_path} ({len(data)} bytes)", file=sys.stderr)

    os.makedirs(os.path.dirname(initrd_path), exist_ok=True)
    with open(initrd_path, 'wb') as f:
        f.write(struct.pack('<I', len(entries)))
        for e in entries:
            f.write(struct.pack('<I', len(e['name'])))
            f.write(e['name'])
            f.write(struct.pack('<I', len(e['path'])))
            f.write(e['path'])
            f.write(struct.pack('<I', len(e['data'])))
            f.write(e['data'])

    total_size = os.path.getsize(initrd_path)
    print(f"\n  Initrd: {len(entries)} archivos, {total_size} bytes", file=sys.stderr)
    print(f"  Archivo: {initrd_path}", file=sys.stderr)
    print(f"  Archivos inyectados: {len(entries)}", file=sys.stderr)

    return {'ok': True, 'count': len(entries), 'total_bytes': total_size}


def main():
    """Lee JSON desde stdin y genera el initrd."""
    if len(sys.argv) > 1:
        json_path = sys.argv[1]
        with open(json_path, 'r') as f:
            params = json.load(f)
    else:
        input_data = sys.stdin.read().strip()
        if not input_data:
            print("ERROR: Se requiere JSON en stdin o como argumento de archivo", file=sys.stderr)
            sys.exit(1)
        params = json.loads(input_data)

    result = build_initrd(params)
    print(json.dumps(result))


if __name__ == '__main__':
    main()
