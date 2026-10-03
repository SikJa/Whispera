# Privacy / Privacidad

## English

Whispera captures the default microphone when you start recording. Audio is written to local disk for
recovery. When you request transcription, audio and active dictionary hints are sent to Groq over HTTPS.
With incremental transcription enabled, completed portions are sent while recording continues.
Cancellation cannot retract portions already sent. Importing sends the selected audio. No audio is sent during the API key check.
Transcripts are stored locally and may be copied/pasted according to your preferences.
The Windows foreground window/control is remembered for paste; this is not a browser-tab integration.

Your Groq key is in Windows Credential Manager (`Whispera.Desktop.Preview`, account `groq`). The application
does not include a shared key. Provider billing, retention and terms are controlled by your Groq account.
Read https://console.groq.com/docs/your-data before using sensitive audio.

Local data: `%APPDATA%\app.whispera.desktop.preview`. Completed/cancelled audio and eligible history older than
48 hours are cleaned hourly when dictation is idle. Pending recordings, settings and dictionary are preserved.
Capture references imported from the other installation continue pointing to its original files.
Uninstalling should not be treated as erasing personal data.
To erase all local data, quit from the tray, back up anything needed, remove that app-data directory and
remove the matching Windows credential. Disable startup in setup before uninstalling.
No automatic update service or Whispera analytics is enabled.

## Español

Whispera captura el micrófono predeterminado al iniciar una grabación y guarda audio en disco para
recuperarlo. Al transcribir envía audio y vocabulario activo a Groq por HTTPS. Importar envía el archivo
elegido. Con transcripción anticipada activa, se envían fragmentos mientras seguís grabando;
cancelar no puede retirar fragmentos ya enviados. La comprobación de clave no envía audio.
Historial y texto permanecen localmente.

La clave es personal y se guarda en Windows Credential Manager (`Whispera.Desktop.Preview`, usuario `groq`).
No se distribuye una clave compartida. Revisá las condiciones y retención de Groq antes de usar audio sensible.

Los datos están en `%APPDATA%\app.whispera.desktop.preview`. Cada hora, sin dictado activo, se limpian
audios completados/cancelados e historial elegible de más de 48 horas; se conservan grabaciones pendientes,
configuración y diccionario. Las capturas importadas siguen en sus rutas originales.
Desinstalar no garantiza borrarlos. Para borrar todo, salí desde la
bandeja, respaldá lo necesario, eliminá esa carpeta y la credencial correspondiente de Windows.
Desactivá el inicio automático desde la guía antes de desinstalar. Sin analíticas ni actualizador automático.

## Clipboard Library / Biblioteca

The library monitors supported Windows clipboard changes and stores history locally in SQLite.
It also records Whispera screenshots and finished screen videos. Nothing in this history is
uploaded to Groq by the library. History is not encrypted. Incognito pauses collection; default
retention is 48 hours and 250 unpinned items. Pins bypass automatic history expiration.
External files are referenced, not deleted by Clear. Owned clipboard PNGs are cleaned hourly.
Known exclusion formats and recognizable API keys are skipped, but not all secrets can be detected.

La biblioteca vigila cambios del portapapeles y guarda texto, links, imágenes y referencias a archivos
localmente. También recibe capturas y videos de Whispera. No envía ese historial a Groq.
No está cifrado: usá incógnito para contenido sensible. Borrar elementos no borra archivos externos.
La detección de secretos es limitada y no reemplaza el cuidado al copiar contraseñas.
Ver detalles y límites en LIBRARY-INTEGRATION.md.

## Video speech / Voz en videos

With video transcription enabled and microphone audio selected, recorded microphone
audio is sent to Groq after stopping. The video images are not sent. Transcription
uses your key, language, model and dictionary; completed segments are checkpointed
for retry. A local `.transcript.txt` can accompany the MP4 in copy/drag operations.
Disable this feature in Clipboard settings before recording sensitive material.

Con transcripción de videos activa y micrófono seleccionado, la voz se envía a Groq
al terminar. No se envían las imágenes del video. Se usa tu clave, idioma, modelo y
diccionario; los fragmentos completados se conservan para reintentar sin repetirlos.
Un `.transcript.txt` local puede acompañar al MP4 al copiar/arrastrar. Desactivá la
opción en Portapapeles antes de grabar contenido sensible.
