# Actualizaciones desde Whispera (K)

Estado: diseño propuesto; todavía no implementado en 0.2.14.

## Experiencia

- Configuración muestra la versión instalada y «Buscar actualizaciones».
- Al abrir la app, una consulta discreta comprueba si hay una versión nueva. Sin consultas constantes ni avisos repetidos.
- Cuando existe una actualización, muestra la versión y una lista corta de cambios con «Actualizar» y «Más tarde».
- «Actualizar» descarga con progreso, verifica la firma, instala y vuelve a abrir Whispera.
- Si hay dictado, transcripción, captura abierta o grabación de video, permite consultar la actualización pero espera a que termine el trabajo antes de instalar.
- Conserva configuración, claves, atajos e historial. Si falla la descarga o la firma, mantiene la versión actual y permite reintentar.

## Distribución

GitHub Releases sigue almacenando cada versión para descargarla y recuperar versiones anteriores. La aplicación consulta un manifiesto estable en `releases/latest/download/latest.json` del repositorio `kazu00001/Whispera-K`.

Usar el plugin oficial de actualizaciones de Tauri 2. Cada instalador se firma con la clave privada del proyecto. La clave pública queda incluida en la app; la privada queda fuera del repositorio, con respaldo seguro, y se utiliza solamente al generar releases. El manifiesto incluye versión, cambios, URL del instalador y firma para Windows x64.

Publicar todos los archivos y comprobarlos antes de marcar la release como última versión. Mantener separadas las versiones preliminares. Un instalador sin firma o un manifiesto incompleto no se anuncian como actualización.

## Primera incorporación

Las versiones actuales no consultan actualizaciones. Sus usuarios necesitan instalar una vez la primera versión que incluya el actualizador. Desde esa versión podrán actualizar desde Configuración.

## Verificación antes de publicarlo

Probar sin actualización, versión nueva, fallo de red, firma inválida, descarga interrumpida, grabación activa y conservación de datos. Probar una actualización real de un instalador firmado a otro en Windows, incluida instalación personalizada y reapertura. El flujo no se considera terminado con una prueba de interfaz simulada.

Referencia: https://v2.tauri.app/es/plugin/updater/
