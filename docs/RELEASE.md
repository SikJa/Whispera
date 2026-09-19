# Release checklist / Lista de publicación

- [x] Separate clean source tree; no personal app-data migration.
- [x] Private GitHub repository requested; public visibility requires another approval.
- [x] NSIS per-user installer with English/Spanish and WebView2 bootstrapper.
- [x] Initial setup, own API key, default microphone detection, shortcut and opt-in autostart.
- [ ] Full settings translation (setup and documentation are bilingual).
- [ ] Code signing identity/certificate (currently unsigned; no purchase authorized).
- [ ] Complete dependency/license review and owner approval of public branding.
- [ ] Fresh Windows installation/uninstallation and restart-at-login validation.
- [ ] Native end-to-end setup with a new test Groq account, without private audio.
- [ ] Microphone signal test, mixed-DPI drag, shortcut conflicts, long capture and recovery on test PC.

CI builds/tests on Windows. Artifacts are private, not a public release. Public downloads should be attached
only after approval, with SHA256SUMS.txt and accurate known limitations. Signing secrets belong in repository
secrets, never in source. Do not enable automatic updates until signing and update verification are designed.

La beta no es una validación completa de producción. No cambiar visibilidad ni publicar una release pública
sin autorización. La instalación diaria del propietario no se reemplaza con esta distribución.
