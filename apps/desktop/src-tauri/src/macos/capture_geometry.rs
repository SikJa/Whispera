//! Pure capture math, also tested on Windows without AppKit or a desktop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Display { pub id: u32, pub x: i32, pub y: i32, pub scale: f64 }
impl Display {
    pub fn local_rect(self, x: i32, y: i32, width: u32, height: u32) -> Result<[f64; 4], String> {
        if !self.scale.is_finite() || self.scale <= 0. || width == 0 || height == 0 {
            return Err("Escala o dimensiones de pantalla invalidas".into());
        }
        Ok([(x as f64 - self.x as f64) / self.scale,
            (y as f64 - self.y as f64) / self.scale,
            width as f64 / self.scale, height as f64 / self.scale])
    }
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
