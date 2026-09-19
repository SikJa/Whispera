# Whispera

Español | [English](README.md)

Dictado de voz para Windows con una grabadora flotante. Pulsá un atajo, hablá y pegá el texto en tu aplicación.
Usa tu propia clave de Groq. No necesita modelos locales, Python ni una cuenta de Whispera.

## Estado

Preparación de beta privada. No publicar hasta la aprobación del propietario.
Windows x64 es el único destino probado para esta versión. El instalador y la guía inicial tienen español
e inglés; el panel principal existente todavía está en español. La traducción completa queda pendiente.
El instalador no está firmado: Windows puede mostrar una advertencia de editor desconocido/SmartScreen.
No desactives las protecciones. Usá solo archivos del repositorio y verificá sus sumas SHA-256.

## Instalar

1. Descargá `Whispera_*_x64-setup.exe` de una versión aprobada.
2. Instalá para tu usuario. Si falta WebView2, el instalador puede descargarlo.
3. La primera apertura muestra una guía. Creá tu clave en https://console.groq.com/keys y validala en la app.
4. Revisá el micrófono predeterminado y los permisos de Windows. Elegí idioma y atajo.
5. Elegí si querés pegado automático e inicio con Windows. Al terminar queda en la bandeja.
6. Seleccioná un campo de texto, pulsá `Control+Shift+Space`, hablá y volvé a pulsarlo.

Las siguientes aperturas quedan ocultas en la bandeja. Desde Configuración podés reabrir la guía.
El pegado intenta recuperar ventana/control nativo, no una pestaña particular del navegador.
Los límites, términos y disponibilidad de Groq aplican; Whispera no garantiza una API gratuita permanente.

## Funciones

- Carpeta transparente arrastrable, tamaño ajustable, paleta personalizada y sonidos.
- Pausa, restauración del sonido del sistema y cancelación con confirmación.
- Whisper Large V3 Turbo o Large V3 mediante Groq.
- Diccionario como ayuda al reconocimiento y correcciones posteriores editables.
- Historial, importar audio, copiar y editar texto plano.
- Audio guardado progresivamente, recuperación y reintentos por fragmentos.
- Inicio con Windows opcional, reinicio manual y vigilancia limitada de interfaz inactiva.

## Privacidad

Se envían audio y vocabulario a Groq al transcribir. Validar la clave solo consulta su lista de modelos.
La clave se guarda en el almacén de credenciales de Windows bajo `Whispera.Desktop`.
Audios, historial, diccionario y registros están en `%APPDATA%\app.whispera.desktop`.
No se cifran con una clave propia de Whispera ni se borran automáticamente, incluso los audios cancelados.
No hay servidor de analíticas de Whispera. [Más información](docs/PRIVACY.md).

## Desarrollo

Requiere Windows, Node.js 22+, Rust MSVC, C++ Build Tools/Windows SDK y WebView2.

```powershell
cd apps/desktop
npm ci
npm run desktop
```

Para compilar: `npm run tauri build -- --bundles nsis`. Para probar Rust: `cargo test --locked` desde `src-tauri`.
Ver [preparación de versiones](docs/RELEASE.md) y [atribuciones](THIRD_PARTY_NOTICES.md).
Licencia MIT para el código propio; se mantienen las licencias de terceros.
