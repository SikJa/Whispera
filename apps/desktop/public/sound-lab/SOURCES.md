# Whispera: candidatos de sonido

Preview local, 2026-09-15. Diez pares (20 WAV). No modifica los sonidos ni las preferencias de la app nativa. El favorito se guarda solamente en el navegador.

## Grabaciones de Handy

Pop y Marimba son los archivos originales, sin modificaciones, del proyecto Handy, MIT, Copyright (c) 2025 CJ Pais. Licencia completa en HANDY-LICENSE.txt, junto a estos WAV.

- Repositorio: https://github.com/cjpais/Handy
- Revision fijada: db1aaac7aaf5cd397da3fed55389d1b7fcec032e
- Archivos: src-tauri/resources/pop_start.wav, pop_stop.wav, marimba_start.wav, marimba_stop.wav.
- Codigo de reproduccion y temas: https://github.com/cjpais/Handy/blob/db1aaac7aaf5cd397da3fed55389d1b7fcec032e/src-tauri/src/audio_feedback.rs

## Propuestas originales

Cristal, Gota, Madera, Seda, Pulso, Orbita, Tecla y Destello se sintetizaron para esta preview. No son grabaciones ni imitaciones exactas de otras marcas. El generador reproducible esta en tools/generate-sound-candidates.mjs; genera PCM mono 16-bit, 44.1 kHz, con envolventes sin cortes y picos limitados. Los WAV de Handy se conservan intactos. El volumen de escucha inicial es 35%; la sonoridad percibida puede variar entre propuestas.

## Referencias de producto

- Wispr Flow: https://docs.wisprflow.ai/articles/6409258247-starting-your-first-dictation (senales de inicio/fin y ajuste de sonidos).
- Superwhisper: https://ai.superwhisper.com/changelog (estilos/opciones de sonidos y senales al comenzar/terminar).
- Handy: codigo citado arriba, con sonidos Start/Stop, temas personalizados y volumen.

Se investigaron los comportamientos. No se extrajeron sonidos propietarios de Wispr Flow ni de Superwhisper. Fin significa detener captura, no confirmar que la transcripcion haya terminado: el aviso de resultado es un tercer evento pendiente de definir.
