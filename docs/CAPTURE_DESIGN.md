# Capturas: decisiones de producto y verificación

## Referencias consultadas

- [Lightshot: atajos](https://app.prntscr.com/en/hotkeys.html): selección, copiar, guardar, cancelar y pantalla completa.
- [Lightshot: tutoriales de edición](https://app.prntscr.com/en/tutorials.html): edición junto al área seleccionada. La referencia visual aportada por el usuario define lápiz, línea, flecha, rectángulo, resaltador, texto, color y deshacer.
- [ShareX: editor](https://getsharex.com/docs/image-editor): deshacer/rehacer, mover y borrar anotaciones, Shift para alinear y edición antes de copiar.
- [CleanShot](https://cleanshot.com/): acceso rápido, anotaciones e historial para recuperar capturas.

Se mantiene la carpeta animada y la configuración existente. Las herramientas
aparecen junto al recorte; el video sigue iniciando al soltar la selección y
terminando con el mismo atajo. No se añade una ventana de confirmar o copiar.
Las capturas de imagen tienen un atajo independiente y se copian como imagen PNG.

Extras consultados y aceptados: historial de las últimas 12 capturas y videos,
guardar correcciones de transcripciones y bloques difuminados de esquinas
redondeadas. No se integran servicios de subida, redes sociales ni búsqueda de
imágenes: requieren destinos y un flujo distinto del uso local solicitado.

## Calidad y funcionamiento

- PNG sin pérdida, con dimensiones físicas del recorte y escala DPI de Windows.
- MP4/H.264, CRF 18, 30 FPS y píxeles yuv420p; AAC 192 kbps cuando hay audio.
  No se reduce la resolución. Las dimensiones impares reciben como máximo un
  píxel adicional para compatibilidad del codificador.
- La tinta se dibuja en una capa capturable. El borde, la carpeta y las
  herramientas están en capas que Windows excluye de la captura.
- Las ventanas esperan al primer cuadro transparente y a su posición final.
  Selección e indicador usan el mismo documento: no hay navegación ni ciclo
  ocultar/mostrar al comenzar a grabar.
- Los trazos se actualizan a ritmo de cuadros de pantalla. El historial de
  edición conserva hasta 80 pasos y comparte trazos inmutables, sin copiar
  imágenes completas por cada movimiento del mouse.
- El bloque difuminado usa una muestra suavizada y opaca del área al aplicarlo.
  En video se mantiene como una máscara fija: tapa los cambios que ocurran debajo,
  sin recapturar toda la pantalla continuamente. No sigue objetos o ventanas.
  Se puede deshacer; los cuadros que ya se grabaron no se modifican retroactivamente.
- El historial conserva referencias a los últimos 12 resultados. Quitar una
  referencia por antigüedad no borra archivos del usuario. Los PNG copiados y los
  MP4 permanecen en la caché de la aplicación para poder volver a copiarlos.
- Las consultas de configuración del dictado ya no cargan también las 2000
  transcripciones y los registros. La revisión de audios pendientes se realiza
  al cambiar de fase o cada tres segundos, en vez de con cada actualización del reloj.
- Al cerrar se desmontan el lienzo, los trazos y las imágenes de fondo; las
  ventanas transparentes se reutilizan. El indicador deja de consultar estado
  cuando está oculto. Las consultas de estado son secuenciales.
- Los atajos se capturan presionando las teclas. Windows libera temporalmente
  los atajos propios mientras el campo escucha y los restaura al salir, perder
  foco o cerrar Configuración. Escape se reserva temporalmente durante capturas
  para poder salir incluso con el foco en otra aplicación.
- El recuadro es blanco por defecto; su color editable se guarda aparte de la carpeta.

## Pruebas

`npm run build`, `cargo test --locked`, `node verify-screen.mjs`,
`node verify-editor.mjs`, `node verify-history.mjs` y `node verify-hotkeys.mjs` (las pruebas web requieren
el servidor local en 127.0.0.1:5190 y simulan IPC).

Los escenarios verifican selección inversa, reutilización sin navegación,
PNG con píxeles de tinta y DPI, deshacer/rehacer, texto, mover/borrar, fallas del
portapapeles, máscara difuminada, recuperación de capturas, edición persistente
de transcripciones y límites de las ventanas y del historial.

La comprobación nativa usa WebView2 local y una superficie sintética propia;
no requiere subir capturas ni transcripciones. La depuración se activa sólo en
el proceso de prueba y se desactiva al abrir la aplicación instalada normalmente.

`verify-native-screen.mjs` verificó PNG de 800×500 con error RGB medio 0 y MP4
de 800×500 con error RGB medio 1,85/255 en la región de comparación, tinta
incluida y controles excluidos. Selección, video e imagen reutilizaron el mismo
documento sin navegar al empezar ni entre capturas.
`verify-native-hotkeys.mjs` usa eventos reales de teclado de Windows: configura
Alt+X sin activar video, restablece el atajo, cancela selección e imagen con
Escape, termina y copia video desde otra ventana y verifica que todas las
capas queden ocultas y el lienzo se libere. Se ejecuta sólo sobre el proceso
local de prueba; requiere que no haya una grabación del usuario en curso.
