# Capturas y video

En Configuración → Capturas y video elegí el audio predeterminado (sin audio,
computadora, micrófono o ambos). Ahí también se muestra el micrófono detectado.
En Configuración → Atajos configurá dictado, video e imagen en un mismo lugar.
Hacé clic en el campo y presioná la combinación: las teclas y los signos «+» se
completan solos. Guardá los cambios; cada función necesita un atajo distinto.
Mientras editás el atajo se suspende su activación para que no empiece a grabar.
Los atajos existentes se conservan. En instalaciones nuevas: **Control+Shift+F9**, sin audio.

1. Pulsá el atajo o «Seleccionar área y grabar».
2. Arrastrá un rectángulo en una pantalla. Al soltar comienza la grabación.
3. Pulsá otra vez el mismo atajo o **Escape**.
4. Esperá a que termine de preparar el video y pegalo con **Ctrl+V**.

Escape cancela la selección o el editor de imagen; durante el video lo termina
y copia, incluso si estás usando otra ventana. No hay botones para detener.
Cada selección pertenece a un monitor; reconoce
su escala de Windows y monitores colocados a la izquierda del principal.
El área queda fija: no sigue a una ventana que se mueva.

Mientras grabás, el borde redondeado sigue marcando el recorte y una luz recorre
su contorno (border beam). Es blanco por defecto, con color configurable para
imagen y video. Respeta la preferencia de movimiento reducido de Windows.
La barra de dibujo queda fija a la derecha y los controles debajo del recuadro;
si falta espacio, se ajustan dentro de los bordes de la pantalla. Los grosores se
eligen con cuatro muestras visuales. La barra se puede contraer; la carpeta
animada conserva tu color y patrón y se puede ocultar para dejar el contador.
La preferencia de carpeta compacta se recuerda. No hay leyenda inferior de atajos.
El borde deja pasar los clics y Windows excluye tanto el borde y su animación
como los controles de la captura. Las esquinas visuales no recortan los archivos:
el PNG y el MP4 conservan todos los píxeles del área rectangular seleccionada.
Al terminar de preparar el MP4, los controles desaparecen automáticamente.

Los controles junto a la carpeta permiten pausar, reanudar y cancelar. Pausar
detiene tanto la imagen como el audio y congela el contador. Al reanudar se crea
otro tramo; al terminar se unen sin recomprimir la imagen. El mismo atajo o
Escape también termina un video pausado. Cancelar descarta únicamente esa sesión,
sin copiarla ni agregarla al historial. El botón del puntero o **V** permite usar
el mouse normalmente después de dibujar; contraer las herramientas también lo hace.

Formato: MP4, H.264 CRF 18, 30 FPS, resolución del recorte (se agrega como máximo un
píxel de borde para dimensiones pares). Audio opcional AAC 192 kbps, 48 kHz estéreo. El audio
de computadora usa el dispositivo de reproducción predeterminado de Windows;
el micrófono usa el dispositivo de entrada predeterminado. Cambiar de dispositivo
durante la grabación no está soportado. No se graba audio sin elegirlo.

Windows copia el MP4 como archivo (CF_HDROP). La aplicación de destino debe
admitir pegar archivos: que acepte imágenes o texto no garantiza que acepte
videos. No se simula Ctrl+V ni se envía el video a un servicio automáticamente.

Los archivos temporales quedan en la carpeta de caché de Whispera,
`screen-recordings/<sesión>/`. No se borran al salir: Windows necesita el
archivo mientras está en el portapapeles. «Ver archivo temporal» permite
encontrarlo y conservarlo o borrarlo manualmente. Si falla la captura o la
codificación, se conservan el video original, el audio y los registros para
recuperación. No hay subida a la nube ni una clave API necesaria para el video.

Por ahora, dictado y grabación de pantalla no funcionan simultáneamente: el
dictado puede silenciar el audio del sistema. No se modifica el dictado existente.

## Capturas y herramientas

El atajo de imagen predeterminado es **Control+Shift+F10** y se configura aparte.
Seleccioná el área, anotá si querés y copiá con **Ctrl+C** o guardá con **Ctrl+S**.
Las imágenes usan PNG sin pérdida y la resolución física del recorte.

La opción **Copiar al soltar la selección**, en Capturas y video, envía el PNG
directamente al portapapeles y cierra el recuadro al soltar el mouse. Está desactivada
inicialmente para conservar el editor. Si el portapapeles falla, se abre la misma
imagen en el editor para poder reintentar sin perder la captura.

Ambos modos incluyen lápiz, línea, flecha, recuadro, resaltador, texto y bloque
difuminado con esquinas redondeadas. **Ctrl+Z** deshace; **Ctrl+Y** o
**Ctrl+Shift+Z** rehace; **Shift** alinea; **V** vuelve al puntero para usar la
pantalla normalmente. El difuminado de video es una máscara fija opaca de una
muestra suavizada; no sigue objetos. Deshacer no cambia cuadros ya grabados.

**Historial → Capturas y videos** permite volver a copiar los últimos 12 PNG/MP4. El historial
de transcripciones permite guardar las correcciones, además de copiar el texto.

## Desarrollo

`npm ci` instala también el binario Windows de FFmpeg a través de ffmpeg-static.
La configuración de Tauri empaqueta el ejecutable, su licencia y los detalles
de compilación. No depende de un FFmpeg instalado globalmente. Requiere Windows,
Node 22+, Rust MSVC, Visual Studio C++ Build Tools y Windows SDK.

Verificaciones: `npm run build`, `cargo test --locked` en `src-tauri`, y
`node scripts/check-source.mjs` desde la raíz. Antes de publicar, probar selección
en monitores con distintas escalas, las cuatro opciones de audio, cancelación,
detención desde el mismo atajo, exclusión del indicador y pegado de MP4.

Para abrir directamente la configuración de una compilación local:
`whispera-desktop.exe --settings`. Cerrá la versión anterior desde su bandeja
antes de abrir otra compilación: Whispera permite una sola instancia.
