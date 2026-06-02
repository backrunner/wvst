use crate::ProtocolError;

pub const PARAMETER_AUTOMATION_EVENT_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParameterAutomationEvent {
    pub sample_offset: u16,
    pub parameter_id: u32,
    pub value_normalized: f64,
}

impl ParameterAutomationEvent {
    pub fn new(
        sample_offset: u16,
        parameter_id: u32,
        value_normalized: f64,
    ) -> Result<Self, ProtocolError> {
        validate_value(value_normalized)?;

        Ok(Self {
            sample_offset,
            parameter_id,
            value_normalized,
        })
    }

    pub fn encode(self, destination: &mut [u8]) -> Result<usize, ProtocolError> {
        if destination.len() < PARAMETER_AUTOMATION_EVENT_LEN {
            return Err(ProtocolError::BufferTooSmall {
                min: PARAMETER_AUTOMATION_EVENT_LEN,
                actual: destination.len(),
            });
        }

        validate_value(self.value_normalized)?;
        destination[..PARAMETER_AUTOMATION_EVENT_LEN].fill(0);
        destination[0..2].copy_from_slice(&self.sample_offset.to_le_bytes());
        destination[4..8].copy_from_slice(&self.parameter_id.to_le_bytes());
        destination[8..16].copy_from_slice(&self.value_normalized.to_le_bytes());

        Ok(PARAMETER_AUTOMATION_EVENT_LEN)
    }

    pub fn decode(source: &[u8]) -> Result<Self, ProtocolError> {
        if source.len() < PARAMETER_AUTOMATION_EVENT_LEN {
            return Err(ProtocolError::BufferTooSmall {
                min: PARAMETER_AUTOMATION_EVENT_LEN,
                actual: source.len(),
            });
        }

        Self::new(
            u16::from_le_bytes([source[0], source[1]]),
            u32::from_le_bytes([source[4], source[5], source[6], source[7]]),
            f64::from_le_bytes([
                source[8], source[9], source[10], source[11], source[12], source[13], source[14],
                source[15],
            ]),
        )
    }
}

fn validate_value(value: f64) -> Result<(), ProtocolError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(ProtocolError::InvalidParameterValue(value));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_automation_event_round_trips() {
        let event = ParameterAutomationEvent::new(12, 42, 0.75).expect("event");
        let mut bytes = [0; PARAMETER_AUTOMATION_EVENT_LEN];

        assert_eq!(event.encode(&mut bytes), Ok(PARAMETER_AUTOMATION_EVENT_LEN));
        assert_eq!(ParameterAutomationEvent::decode(&bytes), Ok(event));
    }

    #[test]
    fn rejects_out_of_range_values() {
        assert!(matches!(
            ParameterAutomationEvent::new(0, 1, 1.5),
            Err(ProtocolError::InvalidParameterValue(_))
        ));
        assert!(matches!(
            ParameterAutomationEvent::new(0, 1, f64::NAN),
            Err(ProtocolError::InvalidParameterValue(_))
        ));
    }
}
