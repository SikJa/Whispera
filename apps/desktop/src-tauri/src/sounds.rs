use crate::storage::Settings;
use windows::{
    core::PCWSTR,
    Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_SYNC},
};
pub fn play(settings: &Settings, stop: bool) {
    if !settings.sounds {
        return;
    }
    macro_rules! pair {
        ($name:literal) => {
            if stop {
                include_bytes!(concat!("../../public/sound-lab/", $name, "-stop.wav")).as_slice()
            } else {
                include_bytes!(concat!("../../public/sound-lab/", $name, "-start.wav")).as_slice()
            }
        };
    }
    let data: &'static [u8] = match settings.sound_theme.as_str() {
        "pop" => pair!("pop"),
        "marimba" => pair!("marimba"),
        "gota" => pair!("gota"),
        "madera" => pair!("madera"),
        "seda" => pair!("seda"),
        "pulso" => pair!("pulso"),
        "orbita" => pair!("orbita"),
        "tecla" => pair!("tecla"),
        "destello" => pair!("destello"),
        _ => pair!("cristal"),
    };
    // Play synchronously in this worker: SND_MEMORY must not be combined with SND_ASYNC.
    std::thread::spawn(move || unsafe {
        let Ok(data) = quiet_wav(data) else {
            return;
        };
        let _ = PlaySoundW(
            PCWSTR(data.as_ptr().cast()),
            None,
            SND_MEMORY | SND_SYNC | SND_NODEFAULT,
        );
    });
}
fn quiet_wav(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut reader =
        hound::WavReader::new(std::io::Cursor::new(data)).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let samples: Vec<f64> = if spec.sample_format == hound::SampleFormat::Float {
        reader
            .samples::<f32>()
            .map(|s| s.map(|v| v as f64))
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?
    } else {
        let max = 2f64.powi(spec.bits_per_sample as i32 - 1);
        reader
            .samples::<i32>()
            .map(|s| s.map(|v| v as f64 / max))
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?
    };
    let mut output = std::io::Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut output,
            hound::WavSpec {
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
                ..spec
            },
        )
        .map_err(|e| e.to_string())?;
        for s in samples {
            writer
                .write_sample((s.clamp(-1., 1.) * 0.35 * 32767.) as i16)
                .map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    Ok(output.into_inner())
}
#[cfg(test)]
mod tests {
    #[test]
    fn selected_sound_is_decodable_at_preview_volume() {
        for data in [
            include_bytes!("../../public/sound-lab/cristal-start.wav").as_slice(),
            include_bytes!("../../public/sound-lab/pop-start.wav").as_slice(),
        ] {
            let wav = super::quiet_wav(data).unwrap();
            assert!(
                hound::WavReader::new(std::io::Cursor::new(wav))
                    .unwrap()
                    .duration()
                    > 0
            );
        }
    }
}
