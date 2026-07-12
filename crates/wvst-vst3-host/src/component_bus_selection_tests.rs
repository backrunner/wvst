use super::*;

#[test]
fn prefers_main_bus_matching_requested_channels() {
    let buses = [
        test_bus(0, 1, true, Vst3BusType::Main),
        test_bus(2, 2, false, Vst3BusType::Main),
    ];

    assert_eq!(
        select_audio_bus_index(&buses, 2, None, Vst3BusDirection::Output).expect("selected"),
        2
    );
}

#[test]
fn uses_explicit_bus_index_when_available() {
    let buses = [
        test_bus(0, 2, true, Vst3BusType::Main),
        test_bus(3, 2, false, Vst3BusType::Aux),
    ];

    assert_eq!(
        select_audio_bus_index(&buses, 2, Some(3), Vst3BusDirection::Output).expect("selected"),
        3
    );
}

#[test]
fn rejects_missing_explicit_bus_index() {
    let buses = [test_bus(0, 2, true, Vst3BusType::Main)];

    let error =
        select_audio_bus_index(&buses, 2, Some(3), Vst3BusDirection::Output).expect_err("bus");

    assert_eq!(
        error,
        HostError::InvalidAudioBusIndex {
            direction: "output",
            index: 3,
        }
    );
}

#[test]
fn rejects_missing_implicit_audio_bus() {
    let error = select_audio_bus_index(&[], 2, None, Vst3BusDirection::Output)
        .expect_err("missing output bus");

    assert_eq!(
        error,
        HostError::InvalidAudioBusIndex {
            direction: "output",
            index: 0,
        }
    );
}

fn test_bus(
    index: i32,
    channel_count: i32,
    default_active: bool,
    bus_type: Vst3BusType,
) -> Vst3AudioBusInfo {
    Vst3AudioBusInfo {
        index,
        direction: Vst3BusDirection::Output,
        channel_count,
        bus_type,
        default_active,
        control_voltage: false,
        name: None,
    }
}
