use super::*;

#[test]
fn prefers_main_bus_matching_requested_channels() {
    let buses = [
        test_bus(0, 1, true, Vst3BusType::Main),
        test_bus(2, 2, false, Vst3BusType::Main),
    ];

    assert_eq!(select_audio_bus_index(&buses, 2), 2);
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
