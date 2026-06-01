use wvst_core::{ChannelCount, FrameCount, SampleRate, StreamId};
use wvst_protocol::{AUDIO_FRAME_HEADER_LEN, AudioFrameFlags, AudioFrameHeader, ProtocolError};

pub fn audio_frame_header_fixture() -> Result<AudioFrameHeader, ProtocolError> {
    AudioFrameHeader::new_f32(
        StreamId::new(42),
        7,
        1_024,
        SampleRate::new(48_000)?,
        FrameCount::new(64)?,
        ChannelCount::new(2)?,
        AudioFrameFlags::SILENCE,
    )
}

pub fn encoded_audio_frame_header_fixture() -> Result<[u8; AUDIO_FRAME_HEADER_LEN], ProtocolError> {
    let mut bytes = [0; AUDIO_FRAME_HEADER_LEN];
    audio_frame_header_fixture()?.encode(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_round_trips() {
        let encoded = encoded_audio_frame_header_fixture().expect("fixture encodes");
        let decoded = AudioFrameHeader::decode(&encoded).expect("fixture decodes");

        assert_eq!(
            decoded,
            audio_frame_header_fixture().expect("fixture builds")
        );
    }
}
