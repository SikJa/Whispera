//! Remove only a long, exactly-zero tail from a derived upload WAV.
//!
//! The durable recording, every nonzero sample, and all earlier timestamps stay
//! intact. Unsupported or ambiguous containers are returned byte-for-byte as-is.

#[cfg(test)]
pub static BENCHMARK_BYPASS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Keep 200 ms after the last nonzero sample, and only shorten tails of at least
/// one second. The caller owns an upload copy; this never opens or writes files.
pub fn trim(bytes: &mut Vec<u8>) {
    #[cfg(test)]
    if BENCHMARK_BYPASS.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let Some((data_header, data_start, sample_rate)) = pcm16_mono_data(bytes) else {
        return;
    };
    let samples = bytes[data_start..].chunks_exact(2);
    let sample_count = samples.len();
    let zero_count = samples
        .rev()
        .take_while(|sample| sample[0] == 0 && sample[1] == 0)
        .count();
    // Whole-file digital silence is handled before upload by the existing guard.
    // Avoid manufacturing a shorter silent recording or an empty container here.
    if zero_count == sample_count || zero_count < sample_rate as usize {
        return;
    }
    let keep = (sample_rate as usize).div_ceil(5);
    let data_len = (sample_count - zero_count + keep) * 2;
    let total_len = data_start + data_len;
    let Ok(data_len_u32) = u32::try_from(data_len) else {
        return;
    };
    let Ok(riff_len_u32) = u32::try_from(total_len - 8) else {
        return;
    };
    bytes[data_header + 4..data_header + 8].copy_from_slice(&data_len_u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&riff_len_u32.to_le_bytes());
    bytes.truncate(total_len);
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

// Parse headers only. Sample inspection starts at the end of the data chunk, so
// a recording that ends in speech does not require a pass over earlier audio.
fn pcm16_mono_data(bytes: &[u8]) -> Option<(usize, usize, u32)> {
    if bytes.get(..4)? != b"RIFF" || bytes.get(8..12)? != b"WAVE" {
        return None;
    }
    if usize::try_from(u32_at(bytes, 4)?).ok()?.checked_add(8)? != bytes.len() {
        return None;
    }
    let mut offset = 12usize;
    let mut rate = None;
    while offset < bytes.len() {
        let chunk_id = bytes.get(offset..offset.checked_add(4)?)?;
        let chunk_len = usize::try_from(u32_at(bytes, offset.checked_add(4)?)?).ok()?;
        let start = offset.checked_add(8)?;
        let end = start.checked_add(chunk_len)?;
        let padded_end = end.checked_add(chunk_len % 2)?;
        if padded_end > bytes.len() {
            return None;
        }
        if chunk_id == b"fmt " {
            // Accept ordinary PCM and its empty extension, but never interpret
            // compressed, extensible, floating-point, or malformed formats.
            if rate.is_some()
                || ![16, 18].contains(&chunk_len)
                || (chunk_len == 18 && u16_at(bytes, start + 16)? != 0)
                || u16_at(bytes, start)? != 1
                || u16_at(bytes, start + 2)? != 1
                || u16_at(bytes, start + 12)? != 2
                || u16_at(bytes, start + 14)? != 16
            {
                return None;
            }
            let sample_rate = u32_at(bytes, start + 4)?;
            if !(8_000..=192_000).contains(&sample_rate)
                || u32_at(bytes, start + 8)? != sample_rate.checked_mul(2)?
            {
                return None;
            }
            rate = Some(sample_rate);
        } else if chunk_id == b"data" {
            // A trailing chunk could carry metadata tied to the original frame
            // count. Leave such WAVs untouched rather than discard that metadata.
            if chunk_len == 0 || chunk_len % 2 != 0 || end != bytes.len() {
                return None;
            }
            return Some((offset, start, rate?));
        } else if chunk_id == b"LIST" {
            // INFO is descriptive text. Other list kinds may contain positions
            // or lengths referring to the original audio, so leave them intact.
            if !info_list(bytes.get(start..end)?) {
                return None;
            }
        } else if chunk_id != b"JUNK" && chunk_id != b"PAD " {
            // Only padding and descriptive INFO metadata are known harmless.
            // In particular, fact/cue /smpl/plst contain counts or positions;
            // unknown chunks may do so too. Preserve those whole WAVs unchanged.
            return None;
        }
        offset = padded_end;
    }
    None
}

fn info_list(bytes: &[u8]) -> bool {
    if bytes.get(..4) != Some(b"INFO") {
        return false;
    }
    let mut offset = 4usize;
    while offset < bytes.len() {
        let Some(size) = offset.checked_add(4).and_then(|at| u32_at(bytes, at)) else {
            return false;
        };
        let Some(end) = usize::try_from(size).ok().and_then(|size| {
            offset
                .checked_add(8)?
                .checked_add(size)?
                .checked_add(size % 2)
        }) else {
            return false;
        };
        if end > bytes.len() {
            return false;
        }
        offset = end;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(bytes: &mut Vec<u8>, id: &[u8; 4], payload: &[u8]) {
        bytes.extend_from_slice(id);
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
        if payload.len() % 2 != 0 {
            bytes.push(0);
        }
    }

    fn finish_riff(bytes: &mut [u8]) {
        let size = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&size.to_le_bytes());
    }

    fn format(rate: u32) -> Vec<u8> {
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&rate.to_le_bytes());
        fmt.extend_from_slice(&(rate * 2).to_le_bytes());
        fmt.extend_from_slice(&2u16.to_le_bytes());
        fmt.extend_from_slice(&16u16.to_le_bytes());
        fmt
    }

    fn wav(rate: u32, samples: &[i16]) -> Vec<u8> {
        let mut bytes = b"RIFF\0\0\0\0WAVE".to_vec();
        chunk(&mut bytes, b"fmt ", &format(rate));
        let pcm: Vec<u8> = samples
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        chunk(&mut bytes, b"data", &pcm);
        finish_riff(&mut bytes);
        bytes
    }

    fn expect_unchanged(mut bytes: Vec<u8>) {
        let original = bytes.clone();
        trim(&mut bytes);
        assert_eq!(bytes, original);
    }

    #[test]
    fn keeps_every_sample_through_last_signal_and_exact_ceil_padding() {
        for rate in [
            8_000, 8_001, 11_025, 16_000, 44_100, 44_101, 48_000, 96_000, 192_000,
        ] {
            let mut samples = vec![0; 37];
            samples.extend([i16::MIN, -1, 0, 1, i16::MAX]);
            samples.extend(vec![0; rate as usize * 2]);
            samples.extend([1, -1]);
            let retained = samples.len();
            samples.extend(vec![0; rate as usize * 2 + 3]);
            let original = wav(rate, &samples);
            let mut result = original.clone();
            trim(&mut result);
            let expected_samples = retained + (rate as usize).div_ceil(5);
            assert_eq!(result.len(), 44 + expected_samples * 2, "rate {rate}");
            assert_eq!(&result[44..], &original[44..44 + expected_samples * 2]);
            assert_eq!(&result[..4], &original[..4]);
            assert_eq!(&result[8..40], &original[8..40]);
            assert_eq!(u32_at(&result, 4), Some((result.len() - 8) as u32));
            assert_eq!(u32_at(&result, 40), Some((expected_samples * 2) as u32));
            let once = result.clone();
            trim(&mut result);
            assert_eq!(result, once, "trimming is idempotent");
        }
    }

    #[test]
    fn threshold_is_one_second_and_a_single_signed_lsb_counts_as_signal() {
        let rate = 8_001;
        for zeros in [0, 1, 1_600, 8_000] {
            let mut samples = vec![-12, 0, 12];
            samples.extend(vec![0; zeros]);
            expect_unchanged(wav(rate, &samples));
        }
        let mut samples = vec![1];
        samples.extend(vec![0; rate as usize]);
        let mut bytes = wav(rate, &samples);
        trim(&mut bytes);
        assert_eq!(bytes.len(), 44 + (1 + (rate as usize).div_ceil(5)) * 2);
        for last in [1, -1, i16::MIN, i16::MAX] {
            let mut samples = vec![0; rate as usize * 3];
            samples.push(last);
            expect_unchanged(wav(rate, &samples));
        }
        expect_unchanged(wav(rate, &vec![0; rate as usize * 3]));
        expect_unchanged(wav(rate, &[]));
    }

    #[test]
    fn preserves_ancillary_chunks_before_data_and_rejects_chunks_after_data() {
        let mut samples = vec![7, -7];
        samples.extend(vec![0; 16_000]);
        let pcm: Vec<u8> = samples
            .iter()
            .flat_map(|value: &i16| value.to_le_bytes())
            .collect();
        let mut bytes = b"RIFF\0\0\0\0WAVE".to_vec();
        chunk(&mut bytes, b"JUNK", b"odd");
        chunk(&mut bytes, b"fmt ", &format(8_000));
        chunk(&mut bytes, b"PAD ", b"padding");
        let mut info = b"INFO".to_vec();
        chunk(&mut info, b"INAM", b"retained metadata\0");
        chunk(&mut bytes, b"LIST", &info);
        let data_header = bytes.len();
        chunk(&mut bytes, b"data", &pcm);
        finish_riff(&mut bytes);
        let original = bytes.clone();
        trim(&mut bytes);
        assert_eq!(&bytes[8..data_header + 4], &original[8..data_header + 4]);
        assert_eq!(&bytes[data_header + 8..], &pcm[..3_204]);
        assert_eq!(u32_at(&bytes, data_header + 4), Some(3_204));
        let mut trailing = original;
        chunk(&mut trailing, b"LIST", b"must not disappear");
        finish_riff(&mut trailing);
        expect_unchanged(trailing);
    }

    #[test]
    fn position_dependent_unknown_and_invalid_metadata_preserve_whole_wav() {
        let mut samples = vec![1i16];
        samples.extend(vec![0; 16_000]);
        let pcm: Vec<u8> = samples
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        for (id, payload) in [
            (*b"fact", 16_001u32.to_le_bytes().to_vec()),
            (*b"cue ", vec![0; 28]),
            (*b"smpl", vec![0; 36]),
            (*b"plst", vec![0; 16]),
            (*b"abcd", vec![]),
            (*b"LIST", b"adtl".to_vec()),
            (*b"LIST", b"wavl".to_vec()),
            (*b"LIST", b"INF".to_vec()),
            (*b"LIST", b"INFOINAM".to_vec()),
            (*b"LIST", b"INFOINAM\xff\xff\xff\xff".to_vec()),
            (*b"LIST", b"INFOINAM\x03\0\0\0abc".to_vec()), // Missing inner padding.
        ] {
            for before_format in [true, false] {
                let mut bytes = b"RIFF\0\0\0\0WAVE".to_vec();
                if before_format {
                    chunk(&mut bytes, &id, &payload);
                }
                chunk(&mut bytes, b"fmt ", &format(8_000));
                if !before_format {
                    chunk(&mut bytes, &id, &payload);
                }
                chunk(&mut bytes, b"data", &pcm);
                finish_riff(&mut bytes);
                expect_unchanged(bytes);
            }
        }
    }

    #[test]
    fn supports_only_consistent_pcm16_mono_headers() {
        let mut samples = vec![1];
        samples.extend(vec![0; 16_000]);
        let base = wav(8_000, &samples);
        for (offset, replacement) in [
            (20, 3u16),   // IEEE float.
            (20, 0xfffe), // Extensible format.
            (22, 0),
            (22, 2), // Stereo is deliberately outside this helper's scope.
            (32, 1),
            (32, 4),
            (34, 8),
            (34, 24),
            (34, 32),
        ] {
            let mut bytes = base.clone();
            bytes[offset..offset + 2].copy_from_slice(&replacement.to_le_bytes());
            expect_unchanged(bytes);
        }
        for rate in [0, 1, 7_999, 192_001, 384_000] {
            expect_unchanged(wav(rate, &samples));
        }
        let mut wrong_byte_rate = base.clone();
        wrong_byte_rate[28..32].copy_from_slice(&15_999u32.to_le_bytes());
        expect_unchanged(wrong_byte_rate);
        for extension in [0u16, 1, 2] {
            let mut bytes = b"RIFF\0\0\0\0WAVE".to_vec();
            let mut fmt = format(8_000);
            fmt.extend_from_slice(&extension.to_le_bytes());
            chunk(&mut bytes, b"fmt ", &fmt);
            chunk(&mut bytes, b"data", &base[44..]);
            finish_riff(&mut bytes);
            if extension == 0 {
                trim(&mut bytes);
                assert_eq!(bytes.len(), 46 + 3_202);
            } else {
                expect_unchanged(bytes);
            }
        }
    }

    #[test]
    fn malformed_or_ambiguous_containers_fail_open_without_mutation() {
        let mut samples = vec![1];
        samples.extend(vec![0; 16_000]);
        let base = wav(8_000, &samples);
        for length in 0..44 {
            expect_unchanged(base[..length].to_vec());
        }
        for length in [base.len() - 2, base.len() - 1] {
            expect_unchanged(base[..length].to_vec());
        }
        for offset in [0, 8] {
            let mut bytes = base.clone();
            bytes[offset] ^= 0xff;
            expect_unchanged(bytes);
        }
        for (offset, size) in [
            (4, 0),
            (4, u32::MAX),
            (16, 15),
            (16, u32::MAX),
            (40, 31_999),
            (40, u32::MAX),
        ] {
            let mut bytes = base.clone();
            bytes[offset..offset + 4].copy_from_slice(&size.to_le_bytes());
            expect_unchanged(bytes);
        }
        let mut unaccounted_tail = base.clone();
        unaccounted_tail.push(0);
        expect_unchanged(unaccounted_tail);
        let mut duplicate_fmt = b"RIFF\0\0\0\0WAVE".to_vec();
        chunk(&mut duplicate_fmt, b"fmt ", &format(8_000));
        chunk(&mut duplicate_fmt, b"fmt ", &format(8_000));
        chunk(&mut duplicate_fmt, b"data", &base[44..]);
        finish_riff(&mut duplicate_fmt);
        expect_unchanged(duplicate_fmt);
        let mut data_first = b"RIFF\0\0\0\0WAVE".to_vec();
        chunk(&mut data_first, b"data", &base[44..]);
        finish_riff(&mut data_first);
        expect_unchanged(data_first);
        let mut odd_data = b"RIFF\0\0\0\0WAVE".to_vec();
        chunk(&mut odd_data, b"fmt ", &format(8_000));
        chunk(&mut odd_data, b"data", &[1, 0, 0]);
        finish_riff(&mut odd_data);
        expect_unchanged(odd_data);
        let mut incomplete_chunk = b"RIFF\0\0\0\0WAVEJUNK".to_vec();
        finish_riff(&mut incomplete_chunk);
        expect_unchanged(incomplete_chunk);
    }

    #[test]
    #[ignore = "Local CPU microbenchmark; use release profile, synthetic PCM only, no IO or network"]
    fn upload_tail_latency() {
        assert!(!cfg!(debug_assertions), "Run this benchmark with --release");
        let rate = 48_000usize;
        for (label, tail) in [
            ("no-silent-tail", 0),
            ("short-tail-200ms", rate / 5),
            ("long-tail-20s", rate * 20),
        ] {
            let mut samples = vec![37i16; rate * 61];
            if tail > 0 {
                let start = samples.len() - tail;
                samples[start..].fill(0);
            }
            let fixture = wav(rate as u32, &samples);
            let mut times = Vec::with_capacity(101);
            for _ in 0..101 {
                // Cloning, fixture generation, and allocation are intentionally
                // excluded. This measures the helper over warm upload bytes.
                let mut payload = fixture.clone();
                let started = std::time::Instant::now();
                trim(std::hint::black_box(&mut payload));
                let elapsed = started.elapsed().as_secs_f64() * 1_000_000.;
                std::hint::black_box(payload.len());
                times.push(elapsed);
            }
            times.sort_by(f64::total_cmp);
            eprintln!("Upload tail helper: {label}, 61s 48kHz mono PCM16, warm bytes, 101 trials; p50={:.3}us p95={:.3}us", times[50], times[95]);
        }
    }
}
