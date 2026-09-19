# Privacy / Privacidad

## English

Whispera captures the default microphone when you start recording. Audio is written to local disk for
recovery. When you request transcription, audio and active dictionary hints are sent to Groq over HTTPS.
Importing sends the selected audio. No audio is sent during the API key check.
Transcripts are stored locally and may be copied/pasted according to your preferences.
The Windows foreground window/control is remembered for paste; this is not a browser-tab integration.

Your Groq key is in Windows Credential Manager (`Whispera.Desktop`, account `groq`). The application
does not include a shared key. Provider billing, retention and terms are controlled by your Groq account.
Read https://console.groq.com/docs/your-data before using sensitive audio.

Local data: `%APPDATA%\app.whispera.desktop`. Audio, history, logs and dictionary are not automatically
deleted. Cancelled audio is retained for safety. Uninstalling should not be treated as erasing personal data.
To erase all local data, quit from the tray, back up anything needed, remove that app-data directory and
remove the matching Windows credential. Disable startup in setup before uninstalling.
No automatic update service or Whispera analytics is enabled.

## Español

Whispera captura el micrófono predeterminado al iniciar una grabación y guarda audio en disco para
recuperarlo. Al transcribir envía audio y vocabulario activo a Groq por HTTPS. Importar envía el archivo
elegido. La comprobación de clave no envía audio. Historial y texto permanecen localmente.

La clave es personal y se guarda en Windows Credential Manager (`Whispera.Desktop`, usuario `groq`).
No se distribuye una clave compartida. Revisá las condiciones y retención de Groq antes de usar audio sensible.

Los datos están en `%APPDATA%\app.whispera.desktop` y no se eliminan automáticamente. Los audios
cancelados también se conservan. Desinstalar no garantiza borrarlos. Para borrar todo, salí desde la
bandeja, respaldá lo necesario, eliminá esa carpeta y la credencial correspondiente de Windows.
Desactivá el inicio automático desde la guía antes de desinstalar. Sin analíticas ni actualizador automático.
