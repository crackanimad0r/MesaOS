#!/bin/bash
# inject_build.sh - Inyecta archivos y reconstruye la ISO en un solo paso
#
# Uso: ./tools/inject_build.sh [archivos/directorios...]
#   Sin argumentos: inyecta desde inyect/ y reconstruye todo
#   Con argumentos: inyecta los archivos/dirs indicados y reconstruye
#
# Ejemplos:
#   ./tools/inject_build.sh                          # inyect/ + build
#   ./tools/inject_build.sh mi_script.sh datos/      # archivos específicos + build

set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Colores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}╔══════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║    MESA OS - INJECT & BUILD SYSTEM      ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════╝${NC}"
echo ""

# Paso 1: Inyectar archivos
echo -e "${YELLOW}[PASO 1/2]${NC} Inyectando archivos al initrd..."
echo ""

INJECT_ARGS=()
if [ $# -eq 0 ]; then
    # Buscar archivos en inyect/ excluyendo README.md
    if [ -d "$ROOT/inyect" ]; then
        while IFS= read -r -d '' file; do
            local basename
            basename=$(basename "$file")
            if [ "$basename" != "README.md" ]; then
                INJECT_ARGS+=("$file")
            fi
        done < <(find "$ROOT/inyect" -type f -print0)
    fi
    if [ ${#INJECT_ARGS[@]} -gt 0 ]; then
        echo -e "${BLUE}[INFO]${NC} Inyectando desde $ROOT/inyect/ (excluyendo README.md)"
        ./tools/inject_to_iso.sh "${INJECT_ARGS[@]}"
    else
        echo -e "${YELLOW}[INJECT]${NC} No hay archivos para inyectar en inyect/."
        ./tools/inject_to_iso.sh
    fi
else
    ./tools/inject_to_iso.sh "$@"
fi

echo ""

# Paso 2: Reconstruir
echo -e "${YELLOW}[PASO 2/2]${NC} Reconstruyendo kernel + ISO..."
echo ""
./build.sh build

echo ""
echo -e "${GREEN}╔══════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  ¡INJECT & BUILD COMPLETADO!            ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════╝${NC}"
echo ""
echo -e "${BLUE}[INFO]${NC} La ISO contiene los archivos inyectados."
echo -e "${BLUE}[INFO]${NC} Al bootear, los archivos estarán en / del RamFS."
echo ""
echo -e "${BLUE}[INFO]${NC} Para ejecutar: ./build.sh run"