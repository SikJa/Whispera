# Whispera (K) 0.1.4 — publicación

Repositorio: https://github.com/kazu00001/Whispera-K

Publicación pública solicitada por el propietario el 2 de octubre de 2026.
Esta versión incluye las modificaciones de captura, video, edición y atajos,
y la corrección del cierre de la carpeta al terminar una transcripción.
Se conservan el historial Git, la licencia y las atribuciones del proyecto original.

## Verificación de esta versión

- Compilación optimizada y empaquetado NSIS para Windows x64 completados.
- 28 pruebas Rust aprobadas; una prueba ignorada.
- Pruebas web de selección, editor, historial y configuración de atajos aprobadas.
- Pruebas nativas de imagen, video y Escape aprobadas en el equipo de desarrollo.
- Dos ciclos reales Alt+Z: el texto queda copiado y guardado, y la carpeta se
  oculta incluso con el aviso de que no había un campo donde pegar.
- El ejecutable instalado coincide con la compilación probada, teniendo en
  cuenta el marcador de empaquetado de Tauri; FFmpeg coincide con la dependencia.

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
