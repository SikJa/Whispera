# Actualizaciones desde Whispera (K)

Implementado desde 0.2.15 para Windows x64.

Configuración → Actualizaciones muestra la versión instalada, los cambios disponibles y «Actualizar y reiniciar». También se accede desde el menú de bandeja, que muestra la versión nueva cuando hay una actualización. La app consulta al iniciar y cada seis horas; la instalación siempre requiere pulsar el botón.

El instalador se descarga desde las releases de este repositorio mediante HTTPS y se verifica con la firma oficial de Tauri. Antes de instalar, se respalda SQLite y se conserva la carpeta de instalación. Whispera vuelve a abrirse al terminar. La clave Groq permanece en el almacén de credenciales de Windows.

No instala durante dictado, transcripción, captura abierta o video. Comprueba de nuevo después de descargar, por si comenzó otro trabajo mientras tanto. Durante la instalación se bloquea el inicio de trabajo nuevo. Los respaldos quedan en `update-backups`, junto a la base de datos del perfil.

## Publicar cambios

1. Hacer commit y push a `main` en `kazu00001/Whispera-K`.
2. «Publicar actualización» calcula una versión estable mayor que las publicadas, sincroniza Cargo y la configuración del build, y ejecuta pruebas.
3. Compila y firma con el secreto cifrado `TAURI_SIGNING_PRIVATE_KEY`.
4. Sube instalador, firma, checksum y `latest.json` a una release provisional. Verifica tamaños y hashes confirmados por GitHub antes de anunciarla como última versión.
5. Las apps la detectan en la siguiente comprobación o al pulsar «Buscar actualizaciones».

Un fallo de compilación o pruebas no se anuncia como actualización. La publicación se ejecuta en serie para conservar el orden. Un workflow repetido para un commit ya publicado no crea otra versión. La versión calculada se aplica al build de ese commit; no genera commits automáticos en `main`.

El workflow habitual de pruebas puede compilar instaladores sin firma; esos archivos no se anuncian como actualizaciones. No reemplazar una release ya publicada: publicar una versión nueva. Para una publicación manual, `scripts/publish-release.mjs` utiliza el instalador firmado y los metadatos `.local/release.json` y `.local/release-notes.md`.

## Claves y primera instalación

La clave pública está incluida en la configuración. La privada está fuera del repo y como secreto cifrado de GitHub. Conservarla: perderla impediría firmar actualizaciones reconocidas por las versiones instaladas. Los archivos `.key` están excluidos del repositorio.

Usuarios con 0.2.14 o anteriores deben instalar una vez la versión con el actualizador. Desde 0.2.15 las siguientes actualizaciones se ofrecen dentro de Whispera.

## Validación

La prueba nativa descarga una pequeña fixture firmada por HTTP local y rechaza bytes alterados; el transporte de producción utiliza HTTPS. Las pruebas de interfaz simulan IPC para cubrir consulta, novedades, errores, progreso y bloqueo durante trabajo activo. La publicación comprueba también los archivos subidos a GitHub antes de habilitar la release.

Referencia oficial: https://v2.tauri.app/plugin/updater/
