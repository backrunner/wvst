use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use wvst_vst3_host::DEFAULT_MAX_VST3_STATE_BYTES;

pub(super) const MAX_CONTROL_BINARY_PAYLOAD_BYTES: usize = DEFAULT_MAX_VST3_STATE_BYTES;
pub(super) const MAX_CONNECTION_MESSAGE_ATTRIBUTES: usize = 64;
pub(super) const MAX_CONNECTION_MESSAGE_ID_BYTES: usize = 128;
pub(super) const MAX_CONNECTION_ATTRIBUTE_KEY_BYTES: usize = 128;
pub(super) const MAX_CONNECTION_ATTRIBUTE_STRING_BYTES: usize = 64 * 1024;
pub(super) const MAX_CONNECTION_ATTRIBUTE_BINARY_BYTES: usize = 1024 * 1024;

pub(super) fn decode_control_base64(label: &'static str, value: &str) -> Result<Vec<u8>, String> {
    decode_base64_limited(label, value, MAX_CONTROL_BINARY_PAYLOAD_BYTES)
}

pub(super) fn decode_message_attribute_base64(
    label: &'static str,
    value: &str,
) -> Result<Vec<u8>, String> {
    decode_base64_limited(label, value, MAX_CONNECTION_ATTRIBUTE_BINARY_BYTES)
}

fn decode_base64_limited(
    label: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    if value.len() > max_base64_len(max_bytes) {
        return Err(format!(
            "{label} exceeds decoded byte limit: max {max_bytes}, encoded length {}",
            value.len()
        ));
    }

    let bytes = BASE64
        .decode(value.as_bytes())
        .map_err(|error| format!("invalid {label}: {error}"))?;
    if bytes.len() > max_bytes {
        return Err(format!(
            "{label} exceeds decoded byte limit: max {max_bytes}, got {}",
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn max_base64_len(max_bytes: usize) -> usize {
    max_bytes.div_ceil(3) * 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_base64_encoded_limit() {
        assert_eq!(max_base64_len(0), 0);
        assert_eq!(max_base64_len(1), 4);
        assert_eq!(max_base64_len(2), 4);
        assert_eq!(max_base64_len(3), 4);
        assert_eq!(max_base64_len(4), 8);
    }

    #[test]
    fn rejects_oversized_encoded_payload_before_decoding() {
        let value = "A".repeat(max_base64_len(1) + 1);

        let error = decode_base64_limited("stateBase64", &value, 1).expect_err("too large");

        assert!(error.contains("stateBase64 exceeds decoded byte limit"));
    }
}
