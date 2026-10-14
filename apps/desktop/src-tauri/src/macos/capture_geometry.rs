//! Device-free capture helpers, also tested on Windows without AppKit or a desktop.
use std::{
    io::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Display { pub id: u32, pub x: i32, pub y: i32, pub scale: f64 }
impl Display {
    pub fn local_rect(self, x: i32, y: i32, width: u32, height: u32) -> Result<[f64; 4], String> {
        if !self.scale.is_finite() || self.scale <= 0. || width == 0 || height == 0 {
            return Err("Escala o dimensiones de pantalla invalidas".into());
        }
        let rect = [(x as f64 - self.x as f64) / self.scale,
            (y as f64 - self.y as f64) / self.scale,
            width as f64 / self.scale, height as f64 / self.scale];
        if rect.iter().any(|value| !value.is_finite()) || rect[0] < 0. || rect[1] < 0. {
            return Err("El area de captura esta fuera de la pantalla".into());
        }
        Ok(rect)
    }
}

/// Swift delivers whole, interleaved stereo Float32 frames, never half a channel.
pub fn stereo_pcm(data: &[u8]) -> Option<Vec<i16>> {
    if data.len() % 8 != 0 { return None; }
    Some(data.chunks_exact(4).map(|bytes| {
        let value = f32::from_ne_bytes(bytes.try_into().unwrap());
        if value.is_finite() { (value.clamp(-1., 1.) * 32767.).round() as i16 } else { 0 }
    }).collect())
}

/// The writer must be nonblocking. Finish an in-flight frame before EOF, but
/// bound even a partial write so a stuck encoder cannot strand the capture thread.
pub fn write_frame(output: &mut impl Write, frame: &[u8], stop: &AtomicBool, timeout: Duration) -> io::Result<bool> {
    let started = Instant::now();
    let mut offset = 0;
    while offset < frame.len() {
        if offset == 0 && stop.load(Ordering::Acquire) { return Ok(false); }
        if started.elapsed() >= timeout { return Err(io::ErrorKind::TimedOut.into()); }
        match output.write(&frame[offset..]) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => offset += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(1)),
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

/// Align a timestamped PCM block to an output timeline without appending overlaps
/// or pre-start samples. All indices/counts are in samples, not bytes.
pub fn pcm_span(end_frames: u64, samples: usize, channels: u16, written: u64) -> (u64, usize) {
    if channels == 0 { return (0, samples); }
    let channels = channels as u64;
    let frames = samples as u64 / channels;
    let begin = end_frames.saturating_sub(frames) * channels;
    let leading = frames.saturating_sub(end_frames) * channels;
    let overlap = written.saturating_sub(begin);
    (begin.saturating_sub(written), (leading + overlap).min(samples as u64) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retina_negative_origin_is_local_points() {
        let display = Display { id: 42, x: -2880, y: -800, scale: 2. };
        assert_eq!(display.local_rect(-2800, -600, 1200, 800).unwrap(), [40., 100., 600., 400.]);
    }
    #[test]
    fn mixed_dpi_overlap_is_disambiguated_by_display() {
        let retina = Display { id: 1, x: 0, y: 0, scale: 2. };
        let external = Display { id: 2, x: 1440, y: 0, scale: 1. };
        assert_eq!(retina.local_rect(1600, 0, 100, 100).unwrap(), [800., 0., 50., 50.]);
        assert_eq!(external.local_rect(1600, 0, 100, 100).unwrap(), [160., 0., 100., 100.]);
        assert_ne!(retina.id, external.id);
    }
    #[test]
    fn invalid_scale_is_rejected() {
        for scale in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(Display { id: 1, x: 0, y: 0, scale }.local_rect(0, 0, 10, 10).is_err());
        }
    }
    #[test]
    fn invalid_rectangles_cannot_reach_screencapturekit() {
        let display = Display { id: 1, x: -100, y: 200, scale: 2. };
        assert!(display.local_rect(-101, 200, 20, 20).is_err());
        assert!(display.local_rect(-100, 199, 20, 20).is_err());
        assert!(display.local_rect(-100, 200, 0, 20).is_err());
        assert!(display.local_rect(-100, 200, 20, 0).is_err());
        let tiny_scale = Display { scale: f64::from_bits(1), ..display };
        assert!(tiny_scale.local_rect(-100, 200, 20, 20).is_err());
    }
    #[test]
    fn extreme_coordinates_are_subtracted_without_integer_overflow() {
        let display = Display { id: 1, x: i32::MIN, y: i32::MIN, scale: 2. };
        assert_eq!(display.local_rect(i32::MAX, i32::MAX, 2, 2).unwrap(),
            [2_147_483_647.5, 2_147_483_647.5, 1., 1.]);
    }
    #[test]
    fn pcm_conversion_preserves_stereo_order_and_sanitizes_nonfinite_values() {
        let bytes: Vec<_> = [-2f32, 2., -0.5, 0.5, f32::NAN, f32::INFINITY].into_iter().flat_map(f32::to_ne_bytes).collect();
        assert_eq!(stereo_pcm(&bytes).unwrap(), [-32767, 32767, -16384, 16384, 0, 0]);
        for len in [1, 4, 7, 9, 12] { assert!(stereo_pcm(&vec![0; len]).is_none()); }
    }
    struct PartialWriter {
        bytes: Vec<u8>,
        calls: usize,
    }
    impl Write for PartialWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            match self.calls {
                1 => Err(io::ErrorKind::Interrupted.into()),
                3 => Err(io::ErrorKind::WouldBlock.into()),
                _ => { let count = bytes.len().min(3); self.bytes.extend_from_slice(&bytes[..count]); Ok(count) }
            }
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    #[test]
    fn frame_writes_retry_partial_interrupted_and_busy_writes_without_losing_bytes() {
        let mut writer = PartialWriter { bytes: Vec::new(), calls: 0 };
        let frame = [1, 2, 3, 4, 5, 6, 7, 8];
        assert!(write_frame(&mut writer, &frame, &AtomicBool::new(false), Duration::from_secs(1)).unwrap());
        assert_eq!(writer.bytes, frame);
    }
    struct BusyWriter<'a> { stop: &'a AtomicBool, cancel: bool }
    impl Write for BusyWriter<'_> {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            if self.cancel { self.stop.store(true, Ordering::Release); }
            Err(io::ErrorKind::WouldBlock.into())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    #[test]
    fn blocked_writer_obeys_stop_and_timeout_without_an_encoder() {
        let stop = AtomicBool::new(false);
        let mut writer = BusyWriter { stop: &stop, cancel: true };
        assert!(!write_frame(&mut writer, &[0; 8], &stop, Duration::from_secs(1)).unwrap());
        stop.store(false, Ordering::Release);
        writer.cancel = false;
        assert_eq!(write_frame(&mut writer, &[0; 8], &stop, Duration::from_millis(5)).unwrap_err().kind(), io::ErrorKind::TimedOut);
    }
    #[test]
    fn stop_before_a_frame_never_writes_partial_pixels() {
        let mut writer = Vec::new();
        assert!(!write_frame(&mut writer, &[0; 8], &AtomicBool::new(true), Duration::from_secs(1)).unwrap());
        assert!(writer.is_empty());
    }
    struct StopDuringFrame<'a> { stop: &'a AtomicBool, bytes: Vec<u8> }
    impl Write for StopDuringFrame<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.stop.store(true, Ordering::Release);
            self.bytes.push(bytes[0]);
            Ok(1)
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    #[test]
    fn stop_finishes_an_already_started_frame_before_eof() {
        let stop = AtomicBool::new(false);
        let mut writer = StopDuringFrame { stop: &stop, bytes: Vec::new() };
        assert!(write_frame(&mut writer, &[0; 8], &stop, Duration::from_secs(1)).unwrap());
        assert_eq!(writer.bytes.len(), 8);
        assert!(!write_frame(&mut writer, &[0; 8], &stop, Duration::from_secs(1)).unwrap());
    }
    #[test]
    fn timestamped_pcm_discards_pre_video_audio_and_overlaps() {
        assert_eq!(pcm_span(100, 400, 2, 0), (0, 200));
        assert_eq!(pcm_span(100, 400, 2, 100), (0, 300));
        assert_eq!(pcm_span(200, 200, 2, 300), (0, 100));
        assert_eq!(pcm_span(200, 200, 2, 400), (0, 200));
    }
    #[test]
    fn timestamped_pcm_pads_gaps_without_changing_channel_alignment() {
        assert_eq!(pcm_span(300, 200, 2, 200), (200, 0));
        assert_eq!(pcm_span(0, 400, 2, 0), (0, 400));
        assert_eq!(pcm_span(20, 100, 0, 0), (0, 100));
    }
}
