//! Timestamp video audio from the device clock, not callback scheduling.
use std::time::{Duration, Instant};

#[cfg(windows)]
#[derive(Default)]
pub struct CaptureClock { anchor: Option<(cpal::StreamInstant, Instant)>, previous:Option<Instant> }
#[cfg(windows)]
impl CaptureClock {
    pub fn end(&mut self, info: &cpal::InputCallbackInfo, samples: usize, rate: u32, channels: u16) -> Instant {
        let stamp = info.timestamp();
        let now = Instant::now();
        let duration=Duration::from_secs_f64(samples as f64 / channels as f64 / rate as f64);
        let age=stamp.callback.duration_since(&stamp.capture);
        if age.is_none_or(|value|value>Duration::from_secs(10)) {
            let end=self.previous.unwrap_or(now)+duration;self.previous=Some(end);return end;
        }
        let (device, real) = *self.anchor.get_or_insert_with(|| {
            let age = stamp.callback.duration_since(&stamp.capture).unwrap_or_default();
            (stamp.capture, now.checked_sub(age).unwrap_or(now))
        });
        let begin = if let Some(elapsed) = stamp.capture.duration_since(&device) {
            real + elapsed
        } else { real.checked_sub(device.duration_since(&stamp.capture).unwrap_or_default()).unwrap_or(real) };
        let end=begin+duration;
        let end=if let Some(previous)=self.previous {
            if end.saturating_duration_since(previous).max(previous.saturating_duration_since(end))>Duration::from_secs(10){previous+duration}else{end}
        }else{end};
        self.previous=Some(end);end
    }
}

/// Device clocks and nanosecond conversion may jitter by a few frames. Never
/// turn that into periodic holes/cuts in contiguous speech. Genuine gaps remain.
pub fn span(end_frames: u64, samples: usize, channels: u16, written: u64, rate: u32) -> (u64, usize) {
    let (silence, skip) = crate::screen_capture::capture_geometry::pcm_span(end_frames, samples, channels, written);
    let tolerance = (rate as u64 / 500).max(1) * channels as u64;
    if written > 0 && silence <= tolerance && (skip as u64) <= tolerance { (0, 0) }
    else { (silence, skip) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn irregular_delivery_preserves_every_sample_and_channel() {
        for rate in [22050, 44100, 48000, 96000] {
            for channels in [1, 2] {
                let frames = rate / 100;
                let mut written = 0;
                for block in 0..1000u64 {
                    let end = (block + 1) * frames as u64;
                    let jitter = if block == 0 { 0 } else { (block % 7) as i64 - 3 };
                    let (pad, skip) = span((end as i64 + jitter) as u64, (frames * channels as u32) as usize, channels, written, rate);
                    assert_eq!((pad, skip), (0, 0), "rate={rate} block={block}");
                    written += frames as u64 * channels as u64;
                }
                assert_eq!(written, frames as u64 * channels as u64 * 1000);
            }
        }
    }
    #[test]
    fn actual_missing_packets_and_pre_start_audio_stay_aligned() {
        assert_eq!(span(960, 960, 2, 480, 48000), (480, 0));
        assert_eq!(span(100, 400, 2, 0, 48000), (0, 200));
        assert_eq!(span(960, 960, 2, 1920, 48000), (0, 960));
    }
}
