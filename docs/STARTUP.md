# Inicio automático en Windows

## Corrección posterior a 0.2.22

Una entrada de inicio podía conservar la ruta de una instalación anterior,
incluso después de desaparecer ese ejecutable. La comprobación anterior solo
verificaba la existencia de la entrada y su habilitación. Además, un proceso
lanzado desde una aplicación empaquetada puede ver un registro privado distinto
del que utiliza Windows al iniciar sesión. No basta con comprobar si el proceso
tiene identidad de paquete para descartar esa diferencia.

Whispera ahora consulta el registro efectivo del usuario mediante el proveedor
de Windows, indicando explícitamente su SID. Al abrir la app, repara la ruta de
una entrada existente y habilitada, conserva su aprobación y verifica el
resultado. Nunca activa por su cuenta una entrada ausente o deshabilitada.
Las rutas se guardan entre comillas y con el argumento `--autostart`.

La opción de configuración y la biblioteca usan el mismo mecanismo. Cada
escritura devuelve una lectura posterior del registro para comprobar el cambio;
un error no se presenta como un guardado correcto. El script es estático, recibe
el comando como dato y se ejecuta sin ventana, sin elevación y sin cambiar la
política de ejecución de PowerShell. La consulta tiene un límite de tiempo.

La comprobación inicial se realiza en segundo plano. Las consultas periódicas
de la biblioteca reutilizan el resultado, incluyendo errores, para no iniciar
procesos continuamente. Abrir la configuración de inicio o cambiar la opción
vuelve a consultar Windows. Esto permite reintentar tras un fallo transitorio.

## Validación

- `cargo test --locked`: políticas de reparación, habilitación/deshabilitación,
  rutas con espacios y Unicode, errores de escritura y estados de aprobación.
- `./apps/desktop/tests/verify-startup-provider.ps1`: proveedor real con claves
  sintéticas aisladas; verifica persistencia, comillas, Unicode, reparación,
  conservación de aprobación y deshabilitación repetida. Limpia sus claves al
  terminar y nunca modifica la entrada real de inicio de Whispera.
- `cargo test --locked startup::tests::real_provider_read_is_non_mutating -- --ignored --nocapture`:
  prueba opcional de lectura real, sin cambiar la configuración del usuario.
- `verify-application.mjs` y `verify-library-settings.mjs`: comportamiento de la
  interfaz con IPC simulado, persistencia y recuperación de errores. Estas
  pruebas no sustituyen una comprobación de inicio de sesión de Windows.

La validación de un arranque completo requiere instalar una compilación que
incluya esta corrección, activar el inicio, cerrar sesión y volver a entrar.
Comprobar el registro y ejecutar pruebas aisladas no demuestra por sí solo ese
recorrido completo. Actualizar el código del repositorio tampoco modifica el
instalador de una versión ya publicada.
