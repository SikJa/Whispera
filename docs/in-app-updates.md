# Actualizaciones de Whispera y novedades de Whispera-K

La app consulta dos fuentes independientes. SikJa/Whispera sigue siendo la unica
fuente de instaladores, verificados con nuestra clave de firma. Whispera-K se
consulta para mostrar publicaciones con cambios que todavia no integramos; no
reemplaza automaticamente la app ni sus ajustes o logo Cristal.

Para K se compara la ultima release estable contra el commit realmente integrado
(`INTEGRATED_COMMIT` en `upstream_updates.rs`), no contra el numero de nuestra
version. Asi dos releases con el mismo numero no ocultan cambios. Al integrar K,
actualizar ese commit junto con el codigo y sus pruebas. Historias divergentes,
fallos de red y limites de GitHub se muestran como consulta no verificada, sin
bloquear la consulta o instalacion de SikJa. No usa tokens de usuario.

Las novedades de K aparecen en Configuracion y no abren ventanas automaticamente.
El boton de K abre exclusivamente su pagina de release; no descarga instaladores.

Implementado desde 0.2.15 para Windows x64.

Configuración → Actualizaciones muestra la versión instalada, los cambios disponibles y «Actualizar y reiniciar». También se accede desde el menú de bandeja, que muestra la versión nueva cuando hay una actualización. La app consulta al iniciar y cada cinco minutos; la instalación siempre requiere pulsar el botón.

El instalador se descarga desde las releases de este repositorio mediante HTTPS y se verifica con la firma oficial de Tauri. Antes de instalar, se respalda SQLite y se conserva la carpeta de instalación. Whispera vuelve a abrirse al terminar. La clave Groq permanece en el almacén de credenciales de Windows.

No instala durante dictado, transcripción, captura abierta o video. Comprueba de nuevo después de descargar, por si comenzó otro trabajo mientras tanto. Durante la instalación se bloquea el inicio de trabajo nuevo. Los respaldos quedan en `update-backups`, junto a la base de datos del perfil.

## Publicar cambios

1. Hacer commit y push a `main` en `SikJa/Whispera`.
2. «Publicar actualización» calcula una versión estable mayor que las publicadas, sincroniza Cargo y la configuración del build, y ejecuta pruebas.
3. Compila y firma con el secreto cifrado `TAURI_SIGNING_PRIVATE_KEY`.
4. Sube instalador, firma, checksum y `latest.json` a una release provisional. Verifica tamaños y hashes confirmados por GitHub antes de anunciarla como última versión.
5. Las apps la detectan en la siguiente comprobación o al pulsar «Buscar actualizaciones».

Un fallo de compilación o pruebas no se anuncia como actualización. La publicación se ejecuta en serie para conservar el orden. Un workflow repetido para un commit ya publicado no crea otra versión. La versión calculada se aplica al build de ese commit; no genera commits automáticos en `main`.

El workflow habitual de pruebas puede compilar instaladores sin firma; esos archivos no se anuncian como actualizaciones. No reemplazar una release ya publicada: publicar una versión nueva. Para una publicación manual, `scripts/publish-release.mjs` utiliza el instalador firmado y los metadatos `.local/release.json` y `.local/release-notes.md`.

## Claves y primera instalación

La clave pública está incluida en la configuración. La privada está fuera del repo y como secreto cifrado de GitHub. Conservarla: perderla impediría firmar actualizaciones reconocidas por las versiones instaladas. Los archivos `.key` están excluidos del repositorio.

Para pasar desde versiones anteriores de SikJa o desde Whispera-K, instalar una vez el instalador de SikJa/Whispera 0.2.23. Las firmas de ambos repositorios son independientes: no cambiar solamente la URL de actualizaciones sin cambiar la clave publica mediante una instalacion de confianza. Desde esta version, las siguientes actualizaciones de SikJa se ofrecen dentro de Whispera.

## Validación

La prueba nativa descarga una pequeña fixture firmada por HTTP local y rechaza bytes alterados; el transporte de producción utiliza HTTPS. Las pruebas de interfaz simulan IPC para cubrir consulta, novedades, errores, progreso y bloqueo durante trabajo activo. La publicación comprueba también los archivos subidos a GitHub antes de habilitar la release.

Referencia oficial: https://v2.tauri.app/plugin/updater/

Desde 0.2.17, la disponibilidad aparece debajo de la marca en la barra lateral. Si Configuración está oculta o minimizada, la app la abre una vez por versión detectada, esperando a que finalicen dictados, transcripciones, capturas y videos activos. La consulta periódica no es una notificación instantánea ni instala automáticamente.
# Mensaje breve para cada versión

Antes de publicar, redactar con el dueño del proyecto el texto de `docs/update-message.txt`. Admite hasta 280 caracteres y cuatro líneas. La publicación usa ese mensaje en `latest.json`, separado del detalle completo del release en GitHub. La app muestra el mensaje sin scroll interno; los manifiestos antiguos con notas largas se resumen para que no ocupen toda la pantalla.
