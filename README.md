<p align="center">
  <img src="apps/desktop/public/cristal/128x128.png" width="80" alt="Whispera" />
</p>

<h1 align="center">Whispera</h1>

<p align="center">
  Dictado por voz, capturas y grabación de pantalla para Windows.<br/>
  Atajos personalizables, edición sobre el recorte y resultados en el portapapeles.
</p>

<p align="center">
  <a href="https://github.com/SikJa/Whispera/releases/latest/download/Whispera-setup.exe"><kbd>⬇ Descargar última versión · Windows</kbd></a>&ensp;·&ensp;
  <a href="README.en.md">English</a>&ensp;·&ensp;
  <a href="https://console.groq.com/keys">Obtener clave Groq</a>&ensp;·&ensp;
  <a href="docs/PRIVACY.md">Privacidad</a>
</p>

<p align="center">El botón descarga directamente el instalador para Windows, sin buscar entre versiones.<br/>Si ya tenés Whispera, buscá la actualización desde <strong>Configuración → Actualizaciones</strong>.</p>

Para pasar desde Whispera-K o una versión anterior de SikJa sin actualizador, instalá esta versión una vez usando el botón. Conserva tu perfil y configura la firma propia de SikJa para futuras actualizaciones.

---

<p align="center">
  <img src="docs/screenshots/recorder-active.png" width="360" alt="Grabadora flotante en acción" />
  &emsp;
  <img src="docs/screenshots/settings-dark.png" width="420" alt="Panel de configuración" />
</p>

## ¿Qué es?

**Whispera** reúne dictado, portapapeles, captura de imágenes y video por región.
Esta actualización integra también el trabajo del fork
[Whispera-K](https://github.com/kazu00001/Whispera-K), conservando su historial y créditos.
El código de Whispera mantiene su licencia MIT; los componentes de terceros
conservan sus respectivas licencias.

Whispera es una app de escritorio para Windows que convierte tu voz en texto usando [Groq](https://groq.com).
Aparece como una **carpeta flotante transparente** sobre cualquier ventana. Grabás, transcribe, y pega el resultado automáticamente — todo desde un atajo de teclado.

- 🎙️ **Sin modelos locales** — usa la API de Groq (Whisper Large V3 Turbo)
- 🔑 **Tu propia clave** — sin cuenta de Whispera, sin servidor nuestro
- 🪟 **Nativo en Windows** — Tauri + Rust + React, con FFmpeg incluido para video
- 🌐 **Español e inglés** — interfaz, instalador y guía inicial bilingüe

## Características

### Novedades de 0.2.22

- **Replay opcional:** guardar los últimos segundos de la pantalla principal, con duración y carpeta elegidas por vos y audio opcional. Después elegís el tramo que querés conservar y podés guardarlo con Ctrl+C. Con audio habilitado, se genera la transcripción del tramo seleccionado.
- **Configuración más cómoda:** inicio con Windows desde la app, Tu espacio desplegable e importación de archivos en una ventana compacta.
- **Correcciones y rendimiento:** recortes de Replay sin perder cuadros al inicio, atajos accesibles, preferencias protegidas durante la carga, recuperación ante errores de captura y menos trabajo repetido en la grabadora y el diccionario.

Replay viene apagado y consume recursos mientras está habilitado. Los detalles de cambios, validación y límites están en [las notas de 0.2.22](docs/releases/0.2.22.md).

La [versión 0.2.6](docs/releases/0.2.6.md) permite mover y redimensionar el
recuadro de capturas y videos desde sus bordes, manteniendo el diseño redondeado
y las anotaciones existentes. En video, el cambio de encuadre tiene una breve transición.

### Novedades de 0.2.0

- **Portapapeles integrado:** texto, enlaces, imágenes, videos y archivos; búsqueda,
  filtros, elementos fijados, grupos y arrastre nativo. Configuración y atajo propios.
- **Editor de imágenes:** combinar capturas en un lienzo, mover y redimensionar,
  fondos originales, color personalizado, difuminado, margen, esquinas y sombra.
- **Captura optimizada:** captura nativa en memoria, menús congelados antes de cambiar
  el foco, barra Línea compacta y más formas. PNG con esquinas transparentes.
- **Video:** captura PNG durante la grabación, controles centrados y procesamiento
  en segundo plano al detener, sin dejar las herramientas bloqueando la pantalla.
- **Voz del video a texto:** transcripción opcional del micrófono con Groq,
  previsualización y copia del texto. El arrastre/copiado puede incluir un TXT junto
  al MP4; que aparezca como texto en el mensaje depende de la aplicación de destino.
- **Dictado largo:** transcripción anticipada por fragmentos mientras seguís hablando,
  con recuperación y reintentos. Requiere conexión y consume la cuota de tu Groq.

Detalles, límites y evidencia de pruebas en [las notas de versión](docs/releases/0.2.0.md).

| | |
|---|---|
| 🎨 **Carpeta personalizable** | Color, escala, posición de controles, animaciones |
| ⏸️ **Pausa y cancelación** | Con confirmación y restauración del audio del sistema |
| 📂 **Transcribir archivos** | Abrí un audio desde el explorador y transcribilo sin grabar |
| 📖 **Diccionario personal** | Nombres y términos que el reconocimiento tiene que respetar |
| 📋 **Historial** | Todas tus transcripciones, editables y copiables |
| 🔊 **Sonidos** | Temas de inicio/fin personalizables (cristal, marimba, pop…) |
| 💾 **Recuperación** | Audio guardado progresivamente, reintentos por fragmento |
| 🚀 **Inicio con Windows** | Arranca oculto en la bandeja del sistema |
| 🎬 **Video por región** | Seleccioná, grabá y terminá con el mismo atajo o Escape; MP4/H.264 a 30 FPS |
| 🖼️ **Capturas PNG** | Resolución nativa sin pérdida; edición o copia directa al soltar |
| ✏️ **Anotaciones** | Lápiz, línea, flecha, recuadro, resaltador, texto y difuminado redondeado; deshacer/rehacer |
| ⌨️ **Atajos al presionar teclas** | Dictado, imagen y video juntos en Configuración → Atajos |
| 🔈 **Audio del video** | Sin audio, computadora, micrófono o ambos; preferencia guardada |
| 🕘 **Capturas recientes** | Volvé a copiar los últimos 12 resultados desde Historial |
| ⏯️ **Controles de video** | Pausar, reanudar y descartar; herramientas fijas al costado y controles debajo del recorte |

El recuadro de captura es blanco por defecto y su color se puede cambiar en
Configuración. La carpeta desaparece al terminar el dictado, incluso cuando no
había un campo donde pegar. Las correcciones del historial de texto se pueden guardar.

El MP4 se copia como archivo: pegarlo con **Ctrl+V** requiere que la aplicación
de destino admita archivos. Se conserva una copia temporal local para sostener
el portapapeles. [Guía de capturas y video](docs/SCREEN_RECORDING.md).

## Instalar

1. Descargá `Whispera_*_x64-setup.exe` desde [Releases](https://github.com/SikJa/Whispera/releases)
2. Instalá para tu usuario — no necesita permisos de administrador
3. La primera vez se abre una **guía de configuración** de 4 pasos:

El paso de Groq incluye una guía ilustrada para obtener la clave. Al completar
el setup, desaparece su acceso; los ajustes siguen disponibles en Configuración.

| Paso | Qué hacés |
|:---:|---|
| 🛡️ | **Privacidad** — Revisás cómo se maneja tu audio y tus datos |
| 🔑 | **Groq** — Creás tu clave en [console.groq.com/keys](https://console.groq.com/keys) y la validás |
| 🎤 | **Preferencias** — Elegís micrófono, idioma, atajo y pegado automático |
| ✅ | **Listo** — Whispera se minimiza a la bandeja, lista para dictar |

4. Hacé clic en un campo de texto, pulsá `Ctrl+Shift+Space`, hablá, y pulsá de nuevo

---

### 🔑 Cómo obtener tu clave de Groq (paso a paso)

<details>
<summary>Ver guía con imágenes</summary>

<br/>

**Paso 1 — Crear una cuenta en Groq**

Andá a [console.groq.com](https://console.groq.com) y registrate con Google o tu email. Es gratis.

<img src="docs/groq-setup/01-login.png" width="520" alt="Página de login de GroqCloud" />

<br/><br/>

**Paso 2 — Ir a API Keys**

Una vez adentro, hacé clic en **API Keys** en el menú de navegación.

<img src="docs/groq-setup/02-api-keys.png" width="520" alt="Página de API Keys en Groq" />

<br/><br/>

**Paso 3 — Crear una nueva clave**

Hacé clic en el botón **Create API Key**.

<img src="docs/groq-setup/03-create-key.png" width="520" alt="Botón Create API Key" />

<br/><br/>

**Paso 4 — Ponerle un nombre**

Escribí un nombre para identificar tu clave (por ejemplo "Whispera") y confirmá.

<img src="docs/groq-setup/04-name-key.png" width="520" alt="Dialog para nombrar la API Key" />

<br/><br/>

**Paso 5 — Copiar la clave**

Tu clave aparece una sola vez. Copiala y pegala en Whispera. Empieza con `gsk_`.

<img src="docs/groq-setup/05-copy-key.png" width="520" alt="Clave API generada para copiar" />

<br/><br/>

> ⚠️ **Importante:** la clave no se vuelve a mostrar. Si la perdés, podés crear una nueva.

</details>

---

## Qué configura cada persona

| Ajuste | ¿Obligatorio? | Detalle |
|---|:---:|---|
| **Clave de Groq** | Sí | Se guarda en Windows Credential Manager, nunca en archivos |
| **Micrófono** | — | Usa el predeterminado de Windows; verificá los permisos |
| **Atajo** | — | `Ctrl+Shift+Space` por defecto, personalizable |
| **Idioma del audio** | — | Español por defecto; inglés, portugués o auto |
| **Pegado automático** | — | Activado por defecto; podés solo copiar |
| **Inicio con Windows** | — | Opcional, arranca en la bandeja |
| **Diccionario** | — | Vacío; agregá tus palabras después |
| **Color y sonidos** | — | Opcionales, hay varios temas incluidos |

## Privacidad

- El audio y el vocabulario del diccionario se envían a **Groq** al transcribir (HTTPS)
- La clave se guarda en **Windows Credential Manager**, no en archivos de texto
- Datos locales en `%APPDATA%\app.whispera.desktop.preview` y `%LOCALAPPDATA%\app.whispera.desktop.preview`
- **No hay servidor de Whispera** — sin analytics, sin telemetría, sin cuenta
- Audio completado/cancelado e historial elegible caducan a las 48 horas; la biblioteca
  tiene sus propios límites y conserva fijados. Usá incógnito para no recoger contenido sensible.

Más información → [docs/PRIVACY.md](docs/PRIVACY.md)

## Desarrollo

Requisitos: Windows, Node.js 22+, Rust stable (MSVC), C++ Build Tools, WebView2.

```bash
cd apps/desktop
npm ci
npm ci --ignore-scripts --prefix vendor/edge-drop
npm run build          # incluye el renderer del portapapeles
npm run desktop        # dev con hot-reload
```

```bash
npm run tauri build -- --bundles nsis    # generar instalador
cd src-tauri && cargo test --locked      # tests de Rust
```

El instalador se genera en `src-tauri/target/release/bundle/nsis/`.

Las pruebas de publicación se ejecutan con `npx playwright install chromium` y
`npm run test:release` después de compilar. Abren un servidor aislado y navegadores
headless silenciados, con archivos sintéticos e IPC simulado. No prueban el
micrófono real ni el pegado en otras aplicaciones. Más detalles en
[CAPTURE_DESIGN.md](docs/CAPTURE_DESIGN.md).

## Licencia

MIT — ver [LICENSE](LICENSE).
Atribuciones de terceros en [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
