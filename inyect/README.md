# Directorio de Inyección de Archivos

Coloca aquí los archivos que quieras inyectar en la ISO de MesaOS.

Estos archivos se empaquetarán en el initrd embebido del kernel y estarán
disponibles en el RamFS durante el boot. **Permanecen en la ISO** incluso
después de reiniciar.

## Cómo usar

1. Coloca tus archivos/directorios aquí:
   ```
   inyect/
   ├── bin/
   │   └── miscript.sh
   ├── etc/
   │   └── micfg.conf
   └── datos/
       └── info.txt
   ```

2. Ejecuta el inyector (sin argumentos usa `inyect/` automáticamente):
   ```bash
   ./tools/inject_to_iso.sh
   ```

   O inyecta archivos específicos:
   ```bash
   ./tools/inject_to_iso.sh ruta/a/mi_archivo
   ./tools/inject_to_iso.sh directorio_completo/
   ```

3. Reconstruye la ISO:
   ```bash
   ./build.sh build
   ```

4. O usa el comando combinado:
   ```bash
   ./tools/inject_build.sh
   ```

   Esto hace `inject_to_iso.sh` + `build.sh build` en un solo paso.

## Notas

- Los archivos se montan en la raíz `/` del RamFS
- La estructura de directorios de `inyect/` se preserva
- Si `inyect/` está vacío, se genera un initrd vacío (sin errores)
- Para añadir SOBRESCRIBIR archivos del sistema (como `/etc/hostname`),
  coloca el archivo en `inyect/` y al bootear sobrescribirá el original