use wvst_core::audio::MAX_CHANNEL_COUNT;

#[derive(Debug)]
pub(super) struct AudioScratchBuffers {
    max_frames: usize,
    input_channels: usize,
    output_channels: usize,
    input: Vec<f32>,
    output: Vec<f32>,
}

impl AudioScratchBuffers {
    pub(super) fn new(
        max_block_frames: u16,
        input_channels: usize,
        output_channels: usize,
    ) -> Result<Self, String> {
        if max_block_frames == 0 {
            return Err("maxBlockFrames must be greater than zero".to_string());
        }
        validate_channel_count("inputChannels", input_channels, true)?;
        validate_channel_count("outputChannels", output_channels, false)?;

        let max_frames = usize::from(max_block_frames);
        let input = vec![0.0; checked_sample_len(max_frames, input_channels)?];
        let output = vec![0.0; checked_sample_len(max_frames, output_channels)?];

        Ok(Self {
            max_frames,
            input_channels,
            output_channels,
            input,
            output,
        })
    }

    pub(super) fn prepare_process<'a>(
        &'a mut self,
        frames: usize,
        payload: &[u8],
    ) -> Result<(&'a [f32], &'a mut [f32]), String> {
        if frames > self.max_frames {
            return Err(format!(
                "frame count exceeds max block size: max {}, got {frames}",
                self.max_frames
            ));
        }

        let input_len = checked_sample_len(frames, self.input_channels)?;
        let output_len = checked_sample_len(frames, self.output_channels)?;
        let expected_payload_len = input_len
            .checked_mul(4)
            .ok_or_else(|| "f32 payload length overflow".to_string())?;
        if payload.len() != expected_payload_len {
            return Err(format!(
                "f32 payload length mismatch: expected {expected_payload_len}, got {}",
                payload.len()
            ));
        }

        for (sample, chunk) in self.input[..input_len]
            .iter_mut()
            .zip(payload.chunks_exact(4))
        {
            *sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        self.output[..output_len].fill(0.0);

        Ok((&self.input[..input_len], &mut self.output[..output_len]))
    }

    pub(super) fn prepare_process_samples<'a>(
        &'a mut self,
        frames: usize,
        samples: &[f32],
    ) -> Result<(&'a [f32], &'a mut [f32]), String> {
        if frames > self.max_frames {
            return Err(format!(
                "frame count exceeds max block size: max {}, got {frames}",
                self.max_frames
            ));
        }

        let input_len = checked_sample_len(frames, self.input_channels)?;
        let output_len = checked_sample_len(frames, self.output_channels)?;
        if samples.len() != input_len {
            return Err(format!(
                "f32 sample length mismatch: expected {input_len}, got {}",
                samples.len()
            ));
        }

        self.input[..input_len].copy_from_slice(samples);
        self.output[..output_len].fill(0.0);

        Ok((&self.input[..input_len], &mut self.output[..output_len]))
    }
}

fn validate_channel_count(
    label: &'static str,
    channels: usize,
    allow_zero: bool,
) -> Result<(), String> {
    if channels == 0 && !allow_zero {
        return Err(format!("{label} must be greater than zero"));
    }
    if channels > usize::from(MAX_CHANNEL_COUNT) {
        return Err(format!(
            "{label} exceeds max channel count: max {}, got {channels}",
            MAX_CHANNEL_COUNT
        ));
    }

    Ok(())
}

fn checked_sample_len(frames: usize, channels: usize) -> Result<usize, String> {
    frames
        .checked_mul(channels)
        .ok_or_else(|| "audio scratch buffer length overflow".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_into_reused_scratch_buffers() {
        let mut buffers = AudioScratchBuffers::new(128, 2, 2).expect("buffers");
        let payload = f32_payload([0.1, 0.2, 0.3, 0.4]);

        let (input, output) = buffers
            .prepare_process(2, &payload)
            .expect("prepared process");
        output.copy_from_slice(input);

        assert_eq!(output, [0.1, 0.2, 0.3, 0.4]);

        let (input, output) = buffers
            .prepare_process(1, &payload[..8])
            .expect("prepared smaller block");
        assert_eq!(input, [0.1, 0.2]);
        assert_eq!(output, [0.0, 0.0]);
    }

    #[test]
    fn rejects_payload_length_mismatch() {
        let mut buffers = AudioScratchBuffers::new(128, 2, 2).expect("buffers");

        let error = buffers
            .prepare_process(2, &[0, 1, 2, 3])
            .expect_err("payload mismatch");

        assert!(error.contains("payload length mismatch"));
    }

    #[test]
    fn prepares_from_interleaved_samples() {
        let mut buffers = AudioScratchBuffers::new(128, 2, 2).expect("buffers");

        let (input, output) = buffers
            .prepare_process_samples(2, &[0.1, 0.2, 0.3, 0.4])
            .expect("prepared samples");

        assert_eq!(input, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(output, [0.0, 0.0, 0.0, 0.0]);
    }

    fn f32_payload<const N: usize>(samples: [f32; N]) -> Vec<u8> {
        let mut payload = Vec::with_capacity(N * 4);
        for sample in samples {
            payload.extend_from_slice(&sample.to_le_bytes());
        }
        payload
    }
}
