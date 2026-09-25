# Transcription while recording / Transcripcion anticipada

## Espanol

Whispera puede enviar lo que ya hablaste a Groq mientras seguis grabando.
La opcion **Transcribir mientras grabo** esta activada por defecto. Se puede
desactivar en Configuracion > Transcripcion para volver al envio al finalizar.

- Procesa intervalos de 60 segundos con un segundo de contexto a cada lado.
  El primer envio puede empezar alrededor del segundo 61.
- Graba el original en disco independientemente de la red y guarda cada resultado.
- Al detener, espera el envio en curso, completa lo pendiente y une el texto.
  El diccionario se aplica una vez y se conserva el flujo de historial/copia/pegado.
- Compara las palabras cercanas al corte en ambos resultados. Si la union es
  ambigua o la cache de un segmento es invalida, procesa el audio completo con
  el metodo anterior. Es una proteccion conservadora, no una garantia de exactitud.
- Audios de menos de 61 segundos y archivos importados conservan el flujo anterior.
- Los tramos no comprimen silencios, para mantener las coordenadas temporales.
- Cancelar o guardar detiene futuros envios, pero no retira audio ya enviado.
  Una peticion en curso todavia puede guardar su respuesta. No se pega texto parcial.
- Errores de red, autenticacion o cuota no activan el reenvio completo como si
  fueran problemas de alineacion. El original queda disponible para reintentar.

### Resultado medido (2026-09-24)

| Medida | Resultado |
| --- | ---: |
| Duracion del audio | 180 s |
| Metodo anterior, mediana de tres solicitudes | 2667.125 ms |
| Anticipado, una reproduccion en tiempo real | 921.027 ms |
| Ahorro observado despues de detener | 1746.098 ms (65.47%) |
| Tramos terminados antes de detener | 2 de 3 |

La prueba anterior del prototipo dio 2681.288 ms frente a 957.077 ms (64.31%).
La segunda prueba uso las protecciones de union y no necesito procesar el audio completo.

Se usaron 12 clips publicos es-ES de [MInDS-14](https://huggingface.co/datasets/PolyAI/minds14),
concatenados y repetidos hasta 180 segundos, convertidos a PCM16 mono de 16 kHz.
Fuente: PolyAI, Gerz et al. 2021, CC-BY-4.0. No fue una charla continua de tres minutos.
No se enviaron audios privados ni diccionarios personales y no se incluyen grabaciones en este repositorio.
Modelo: `whisper-large-v3-turbo`, idioma `es`, temperatura 0, solicitudes reales a Groq.
Se midio preparacion, subida, respuesta y union; no el pegado en otra aplicacion.
El metodo anterior recorta silencios; el anticipado preserva tiempos.
La red y el proveedor pueden variar. Las transcripciones difieren y no hubo una
referencia completa revisada manualmente: no se afirma igual o mejor precision.

## English

**Transcribe while recording** sends completed portions to Groq in the background.
It is enabled by default and can be disabled in Settings > Transcription.
Each 60-second core includes one second of surrounding context when available.
The first upload becomes ready around second 61. The original PCM remains on disk;
successful parts are cached independently of capture. Stopping waits for in-flight
work and completes the tail before applying corrections and publishing once.

Neighboring word signatures are checked at joins. Ambiguous alignment or invalid
segment caches fall back to whole-audio processing. This is a heuristic, not an
accuracy guarantee. Recordings shorter than 61 seconds and file imports use the
previous pipeline. Silence compression is disabled for incremental parts to retain
timestamp coordinates. Network/authentication/quota failures do not trigger a
whole-audio resend under the guise of an alignment problem.

Cancellation/save prevents future uploads but cannot retract submitted audio.
An in-flight response may still be cached. Partial text is never auto-pasted.
See [Privacy](PRIVACY.md).

The 2026-09-24 public-fixture test measured **2667.125 ms** baseline median (three
requests) versus **921.027 ms** incremental (one real-time 180-second replay):
**1746.098 ms / 65.47%** less waiting after stop, with no fallback needed.
The earlier prototype measured 64.31%. These are small controlled measurements,
not guarantees. Twelve public Spanish MInDS-14 clips were concatenated/repeated,
not naturally continuous dictation. See source/license and method above.
Transcriptions differed; no accuracy improvement is claimed. Clipboard/focus
restoration was excluded from the measured latency.

## Verification

- Offline Rust tests cover sample ranges, boundary ownership, ambiguous joins,
  cancellation, cache recovery, dictionary application, short-audio fallback and
  backward-compatible settings. Existing audio/dictionary/storage tests remain.
- The real API benchmark is ignored by default and requires explicit opt-in,
  FFmpeg, the public fixture WAVs and a locally configured Groq credential.
  No credential or fixture is bundled. Do not enable this test in CI.
- The local desktop implementation also passed microphone pause/resume/cancel
  checks without uploading the microphone recording, plus cached public-fixture
  recovery and history idempotency checks.

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked
```

The opt-in harness is `incremental::tests::benchmark_three_minutes_public`.
It expects the public fixture files `sample-000.wav` through `sample-099.wav`
(indices 0, 9, ..., 99) under `.local/benchmarks/groq-public-es-20260915/`.
Set `WHISPERA_RUN_PUBLIC_BENCHMARK=1` and run that test with `--ignored --nocapture`
only after preparing these public clips. Ordinary tests make no API requests.

Provider documentation: https://console.groq.com/docs/speech-to-text
