# Transcription and recorder performance validation

Changes preserve the Cristal artwork, updater identity, selected model and stored preferences.

## Changes

- Upload eligible 16-bit WAV audio as lossless FLAC, preserving rate, channels and samples. Conversion runs on a blocking worker, has a bounded lifetime and falls back to the original bytes when unavailable or unsuccessful. Small/unsupported sources retain their original format. No extra transcription request is added.
- Wake incremental dictation immediately on stop, save, cancel or recorder failure. Already submitted requests can finish caching; a wake does not discard their result.
- Request segment timestamps in the existing video transcription call. Export phrase intervals in the TXT and explicit transcript copy. Preserve plain text when segment metadata is incomplete or conflicts with dictionary corrections. Older cached transcripts remain readable.
- Track encoded part durations and WAV lengths across paused/reframed recordings. Replay timestamps refer to the retained clip. Completed chunks survive retries and repeated exports retain timestamps.
- Wake the durable video queue on enqueue/retry instead of waiting for its one-second recovery poll.
- Compile dictionary rules once for the transcript and its segments, retaining replacement order and Unicode boundaries.
- Reuse the native frozen crop for unmarked screenshots. Annotated captures still render all marks. Prepare rounded pixels once; publish the already encoded lossless PNG plus an alpha-preserving bottom-up DIBV5 on Windows instead of invoking another PNG encoder while holding the clipboard. Preserve other-platform behavior. An independent in-flight export guard prevents repeated Ctrl+C from queuing copies when resize readiness changes. A canceled session is checked again before publishing. Numeric preparation/clipboard/close timings are logged without image content.
- Suspend only hidden UI status polling. Background capture/transcription continue. Recorder hit regions update on geometry changes instead of scanning the idle DOM every 50 ms. Visibility/focus resumes polling immediately; overlapping calls are coalesced.

## Measurements

Windows 11; sequential synthetic Spanish audio; Groq `whisper-large-v3-turbo`; no personal recordings. Warm-up pairs excluded from medians, measured pairs alternate order. These are observations on one machine/network, not guaranteed service latency.

| Case | Before | After | Scope |
|---|---:|---:|---|
| Stop to text, 20 s mono 48 kHz | 1,435.6 ms | 945.7 ms | Five alternating pairs; includes worker stop, silence trim, materialization, upload, API and durable result. Excludes physical microphone flush and desktop paste. 34.1% reduction. |
| WAV versus lossless upload, 20 s 22.05 kHz | 1,039.5 ms | 621.7 ms | Six alternating pairs; includes conversion. Payload 882,044 to 178,217 bytes, 79.8% less. |
| Plain versus segment response | 531.5 ms | 549.5 ms | Six alternating pairs with production lossless upload in both paths. 18 ms median difference is within observed request variation. |
| Four texts, 100 dictionary rules | 292.1 ms | 73.4 ms | Eight local repetitions; output equality asserted. |
| Four texts, 500 dictionary rules | 1,491.1 ms | 373.3 ms | Eight local repetitions; output equality asserted. |
| Idle recorder geometry reads, 4 s | 480 | 0 | Real frontend in headless Chromium with native IPC mocked. Not a CPU percentage. |
| Hidden recorder state/settings calls, 4 s | 3 / 3 | 0 / 0 | Same mocked UI harness. Visible polling frequency preserved. |
| Video queue dispatch | 250–1,001 ms | 0.03–0.12 ms | Local wake primitive comparison at four phases of the previous poll. Excludes encoding and ASR. |
| Screenshot preparation, synthetic 1920×1080 | 65.3 ms | 20.7 ms | Three measured release-mode repetitions after warm-up. Native conversion path only; excludes desktop capture/clipboard write. Every output pixel, including alpha, compared. |
| Screenshot preparation, synthetic 3840×2160 | 261.2 ms | 81.2 ms | Same conditions; 68.9% reduction. Unmarked images also skip browser PNG/JSON conversion, which this native benchmark excludes. |

The 65-second paced synthetic dictation cached its first complete chunk before stopping, retained final speech and finished 550 ms after the producer stopped. A cached retry took under 1 ms. No fallback was needed in that run.

All paired short transcription outputs were identical between formats/modes. The audio roundtrip regression decodes FLAC and compares every original stereo 48 kHz PCM sample. These checks establish no regression on the supplied fixtures; they do not establish universal ASR accuracy or exact word/frame alignment.

## Validation

- 161 ordinary Rust tests passed; 26 opt-in tests were skipped by the ordinary run. Selected network/media/performance and isolated Windows clipboard tests were executed separately. The private clipboard test also validates exact PNG pixels, DIB bytes and removal of stale file/text formats for the new publisher.
- 34 headless frontend suites passed, including replay trimming, autosave, hotkeys, frozen capture, error recovery, updates, window lifecycle, performance and the new copy/export regression. Native IPC is mocked in these suites. Copy tests cover errors/retry, repeated Ctrl+C, resizing during an in-flight copy, canceled save and preservation of composed annotations.
- Optimized production compilation and local NSIS installation succeeded. The installed code matched the tested binary, allowing only Tauri's documented three-byte NSIS bundle marker.
- Real installed Windows UI: synthetic audio import/result, recorder open/close, Print Screen selection/copy producing a valid 570×540 PNG, a silent video recording/shortcut stop producing a decodable MP4, update check, and preference/data preservation.
- After the copy changes, three additional real Windows captures copied and closed successfully. Native preparation/clipboard/close phases were 1/2/2 ms, 1/2/2 ms and 0/0/1 ms (integer-millisecond logging; excludes frontend and key delivery). The third had a drawn arrow: its 585×510 exported PNG retained the arrow pixels. No stall reproduced in these three attempts; this sample cannot exclude intermittent contention in other applications.
- SQLite integrity remained valid. Existing settings, transcript IDs/content and library IDs/content were preserved. The clipboard monitor refreshed usage metadata for an existing item on restart, as implemented by the existing library deduplication path.

## Limits

Protected capture toolbars/frame surfaces are excluded from Windows capture. The automation helper returned missing input geometry for their buttons. Their exact visible glow and drag behavior require a human visual check; the headless geometry/resize suites passed. Physical microphone-to-paste timing, Codex text insertion, multi-hour stability, system sleep/restart and native macOS were not validated here. Never infer a global 50–90% improvement, zero bugs, or guaranteed sub-second transcription from these results.

Startup prewarming and the chosen capture encoder were retained: reducing them could trade screenshot responsiveness or video quality for lower idle memory and requires separate comparable measurements.

## Reproduction

Run the ordinary release/native suites. Opt-in tests require explicit synthetic fixture paths and authorization for Groq uploads; they never default to a user's recordings. Relevant tests: `synthetic_short_dictation_stop_to_text`, `synthetic_long_dictation_preserves_completed_chunks_and_final_tail`, `synthetic_segment_latency`, `synthetic_lossless_upload_latency`, `synthetic_dictionary_segments_latency`, `synthetic_timed_video_and_trim_pipeline`, and `private_station_clipboard_replaces_stale_formats`. The network-free `verify-performance.mjs` runs with the other release frontend suites.
