# Whispera (K) 0.1.5 — publicación

Repositorio: https://github.com/kazu00001/Whispera-K

Publicación pública solicitada por el propietario el 2 de octubre de 2026.
Esta versión agrega pausa, reanudación y cancelación de video, controles movibles
y compactos, copia de PNG al soltar la selección y la guía ilustrada dentro del
setup. Reúne los tres atajos en una sección y las capturas recientes en Historial.
El setup completado deja de aparecer como acceso en Configuración.
Se conservan el historial Git, la licencia y las atribuciones del proyecto original.

## Verificación de esta versión

- Compilación optimizada y empaquetado NSIS para Windows x64 completados.
- 29 pruebas Rust aprobadas; la prueba nativa opcional se ejecutó por separado
  y aprobó las cuatro opciones de audio.
- Pruebas web de selección, editor, historial, atajos y setup aprobadas.
- PNG nativo de 800 × 500 idéntico al patrón de referencia; MP4 de igual tamaño
  con error RGB medio de 1,85/255 en el área comparada. Anotaciones incluidas y
  controles excluidos. La selección reutilizó el documento sin navegar.
- Pruebas nativas de pausa/reanudación con y sin audio, finalización desde pausa,
  cancelación grabando y pausado, y copia automática de PNG aprobadas. Ningún
  cuadro del intervalo pausado apareció en el video terminado.
- Intercambio de los atajos de imagen/video y rechazo de duplicados verificados;
  preferencias del usuario restauradas al terminar las pruebas.
- El ejecutable instalado coincide con la compilación probada, teniendo en
  cuenta el marcador de empaquetado de Tauri; FFmpeg coincide con la dependencia.

Instalador: `Whispera_0.1.5_x64-setup.exe` (28.469.622 bytes).
SHA-256: `4e3515ca8315de1c8ecb53c566986efc0f6e34e3bc9aec03c72b21951de77819`.

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
