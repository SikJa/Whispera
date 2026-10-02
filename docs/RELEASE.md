# Whispera (K) 0.1.6 — publicación

Esta versión redondea el recuadro de captura y agrega una luz que recorre el
borde. Conserva el color configurable, la resolución original y la exclusión
del indicador del archivo exportado. El movimiento reducido desactiva el efecto.

Las herramientas quedan fijas en una barra vertical a la derecha. Los controles
de imagen y video se anclan debajo de la selección y no se desplazan al contraer
la barra o la carpeta. Los grosores se eligen mediante muestras visuales y los
botones usan el estilo redondeado de la grabadora.

Se corrigió el bloqueo al intentar cambiar de herramienta después de dibujar:
las ventanas de controles ahora pertenecen a la ventana de anotaciones, por lo
que Windows mantiene sus botones por encima del lienzo.

## Verificación

- Compilación TypeScript/Vite y 30 pruebas Rust aprobadas.
- Cinco suites de interfaz aprobadas: selección, edición, historial, atajos y setup.
- Captura nativa PNG 800×500 idéntica a la referencia (error RGB medio 0).
- MP4 800×500 con error RGB medio 1,85/255 fuera de la anotación; marca incluida,
  indicador y controles excluidos, sin recargar el selector entre capturas.
- Pausa, reanudación, detención desde pausa, cancelación y copia automática de PNG
  comprobadas en Windows; no se incluyeron cuadros del intervalo pausado.
- Posición de ambos paneles verificada en bordes, orígenes negativos y escalas de
  100, 125, 150 y 200 %. Contraer no desplaza el control que permite expandir.
- El cambio de herramienta y sus acciones se verificaron con eventos e IPC; la
  comprobación final con clics físicos quedó interrumpida por Escape. La herramienta
  de automatización también tuvo problemas para dirigir clics a ventanas secundarias.
- Ejecutable instalado igual a la compilación probada salvo el marcador de
  empaquetado de Tauri. Preferencias e historial del usuario restaurados.

Instalador: `Whispera_0.1.6_x64-setup.exe` (28.475.324 bytes).
SHA-256: `82719195bacfe16b837abd9427070dc24d9dbeef666c6dd903f76ad02a54d5f9`.

## Distribución

El código no incluye claves, datos de la aplicación, grabaciones, bases de datos,
dependencias instaladas ni archivos de compilación. El instalador y su SHA-256
se distribuyen como archivos adjuntos de una release, fuera del historial Git.

El instalador no tiene firma de código. FFmpeg se distribuye con su propia
licencia y referencias a su código fuente; ver [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).

## Comprobaciones pendientes del proyecto original

- [x] Separate clean source tree; no personal app-data migration.
- [x] Public GitHub repository explicitly requested by the owner.
- [x] NSIS per-user installer with English/Spanish and WebView2 bootstrapper.
- [x] Initial setup, own API key, default microphone detection, shortcut and opt-in autostart.
- [ ] Full settings translation (setup and documentation are bilingual).
- [ ] Code signing identity/certificate (currently unsigned; no purchase authorized).
- [ ] Complete dependency/license review and owner approval of public branding.
- [ ] Fresh Windows installation/uninstallation and restart-at-login validation.
- [ ] Native end-to-end setup with a new test Groq account, without private audio.
- [ ] Microphone signal test, mixed-DPI drag, shortcut conflicts, long capture and recovery on test PC.

CI builds/tests on Windows. Public downloads include SHA256SUMS.txt and known
limitations. Signing secrets belong in repository secrets, never in source.
Automatic updates remain disabled until signing and update verification are designed.
