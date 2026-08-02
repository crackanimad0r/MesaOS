#!/bin/bash
# inject_to_iso.sh - Inyector de archivos a la ISO de MesaOS
# Toma archivos/directorios y los empaqueta en un initrd binario
# (output/initrd.bin) que el kernel enlaza via include_bytes!.
#
# Los archivos inyectados se montan en el RamFS durante el boot
# y PERMANECEN en la ISO. No se borran al reiniciar.
#
# Uso: ./tools/inject_to_iso.sh [archivos/directorios...]
#   Sin argumentos: inyecta desde el directorio 'inyect/'
#
# Ejemplos:
#   ./tools/inject_to_iso.sh mi_script.sh datos/
#   ./tools/inject_to_iso.sh                    # usa inyect/

set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Colores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

OUTPUT_DIR="$ROOT/output"
mkdir -p "$OUTPUT_DIR"

INJECT_DIR="$ROOT/inyect"
INITRD_FILE="$OUTPUT_DIR/initrd.bin"
PY_SCRIPT="$ROOT/tools/inject_to_iso.py"

# ──────────────────────────────────────────────────────────────────
# 1. RECOLECTAR ARCHIVOS
# ──────────────────────────────────────────────────────────────────

declare -a FILE_LIST=()
USE_DEFAULT=false
INJECT_DIR_ABS=""

if [ $# -eq 0 ]; then
    if [ -d "$INJECT_DIR" ] && ls "$INJECT_DIR"/* &>/dev/null 2>&1; then
        echo -e "${BLUE}[INJECT]${NC} Usando directorio: $INJECT_DIR"
        while IFS= read -r -d '' file; do
            FILE_LIST+=("$file")
        done < <(find "$INJECT_DIR" -type f -print0)
        USE_DEFAULT=true
        INJECT_DIR_ABS=$(realpath "$INJECT_DIR")
    fi
else
    for arg in "$@"; do
        if [ -f "$arg" ]; then
            FILE_LIST+=("$(realpath "$arg")")
        elif [ -d "$arg" ]; then
            while IFS= read -r -d '' file; do
                FILE_LIST+=("$file")
            done < <(find "$(realpath "$arg")" -type f -print0)
        else
            echo -e "${RED}[ERROR]${NC} No existe: $arg"
            exit 1
        fi
    done
fi

# ──────────────────────────────────────────────────────────────────
# 2. GENERAR JSON CON LISTA DE ARCHIVOS
# ──────────────────────────────────────────────────────────────────

# Crear archivo JSON temporal
JSON_FILE=$(mktemp /tmp/mesainject_XXXXXX.json)
trap 'rm -f "$JSON_FILE"' EXIT

# Construir JSON manualmente - más fiable que heredocs con expansión
# Escribir array de files
echo -n '{"files":[' > "$JSON_FILE"
FIRST=true
for f in "${FILE_LIST[@]}"; do
    if [ "$FIRST" = true ]; then
        FIRST=false
    else
        echo -n ',' >> "$JSON_FILE"
    fi
    # Escapar para JSON: \, ", \n, \t, \r
    ESCAPED=$(printf '%s' "$f" | python3 -c "import sys,json; print(json.dumps(sys.stdin.read().rstrip('\n')))")
    echo -n "$ESCAPED" >> "$JSON_FILE"
done
echo -n '],' >> "$JSON_FILE"

# use_default (booleano)
echo -n '"use_default":' >> "$JSON_FILE"
if [ "$USE_DEFAULT" = true ]; then
    echo -n 'true,' >> "$JSON_FILE"
else
    echo -n 'false,' >> "$JSON_FILE"
fi

# inject_dir
ESCAPED_DIR=$(printf '%s' "$INJECT_DIR_ABS" | python3 -c "import sys,json; print(json.dumps(sys.stdin.read().rstrip('\n')))")
echo -n '"inject_dir":' >> "$JSON_FILE"
echo -n "$ESCAPED_DIR," >> "$JSON_FILE"

# initrd_path
ESCAPED_PATH=$(printf '%s' "$INITRD_FILE" | python3 -c "import sys,json; print(json.dumps(sys.stdin.read().rstrip('\n')))")
echo -n '"initrd_path":' >> "$JSON_FILE"
echo -n "$ESCAPED_PATH" >> "$JSON_FILE"

echo '}' >> "$JSON_FILE"

# ──────────────────────────────────────────────────────────────────
# 3. MOSTRAR ARCHIVOS Y EJECUTAR
# ──────────────────────────────────────────────────────────────────

echo -e "${BLUE}[INJECT]${NC} Archivos a empaquetar:"
if [ ${#FILE_LIST[@]} -eq 0 ]; then
    echo "  (ninguno - initrd vacío)"
else
    for f in "${FILE_LIST[@]}"; do
        echo "  - $f"
    done
fi
echo ""

echo -e "${BLUE}[INJECT]${NC} Procesando..."
RESULT=$(python3 "$PY_SCRIPT" "$JSON_FILE" 2>&1)

# La salida puede contener mensajes de debug y una línea JSON final
# Buscar línea JSON con 'ok':true
PY_OK=$(echo "$RESULT" | grep '{"ok":' | tail -1)

if [ -z "$PY_OK" ]; then
    echo "$RESULT"
    echo ""
    echo -e "${RED}[ERROR]${NC} Fallo en generación del initrd."
    exit 1
fi

# Mostrar líneas que no sean el JSON final de resultado
echo "$RESULT" | grep -v '^{"ok":' || true

COUNT=$(echo "$PY_OK" | python3 -c "import sys,json; print(json.load(sys.stdin)['count'])")

echo ""
echo -e "${GREEN}[OK]${NC} Initrd generado correctamente ($COUNT archivos)."
echo -e "${BLUE}[INFO]${NC} $INITRD_FILE"
echo ""
echo -e "${BLUE}[IMPORTANTE]${NC} Recompila el kernel para que los cambios surtan efecto:"
echo -e "  ${YELLOW}./build.sh build${NC}  o  ${YELLOW}./tools/inject_build.sh${NC}"
echo ""
echo -e "${BLUE}[NOTA]${NC} Los archivos inyectados se montan en / durante el boot"
echo -e "${BLUE}${NC} y PERMANECEN en la ISO (no se pierden al reiniciar)."