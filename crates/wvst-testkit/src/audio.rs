pub fn silence(frames: usize, channels: usize) -> Vec<f32> {
    vec![0.0; frames.saturating_mul(channels)]
}

pub fn interleaved_sine(
    frames: usize,
    channels: usize,
    frequency_hz: f32,
    sample_rate_hz: f32,
) -> Vec<f32> {
    let mut output = Vec::with_capacity(frames.saturating_mul(channels));

    for frame in 0..frames {
        let phase = frame as f32 * frequency_hz * std::f32::consts::TAU / sample_rate_hz;
        let sample = phase.sin();

        for _ in 0..channels {
            output.push(sample);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_silence() {
        assert_eq!(silence(2, 2), vec![0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn creates_interleaved_sine_shape() {
        let samples = interleaved_sine(3, 2, 440.0, 48_000.0);

        assert_eq!(samples.len(), 6);
        assert_eq!(samples[0], samples[1]);
        assert_eq!(samples[2], samples[3]);
    }
}
