use crate::vst3_abi::{
    BusInfo, VST3_BUS_DIRECTION_INPUT, VST3_BUS_DIRECTION_OUTPUT, VST3_BUS_FLAG_CONTROL_VOLTAGE,
    VST3_BUS_FLAG_DEFAULT_ACTIVE, VST3_BUS_TYPE_AUX, VST3_BUS_TYPE_MAIN,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Vst3BusDirection {
    Input,
    Output,
}

impl Vst3BusDirection {
    pub(crate) const fn as_abi(self) -> i32 {
        match self {
            Self::Input => VST3_BUS_DIRECTION_INPUT,
            Self::Output => VST3_BUS_DIRECTION_OUTPUT,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Vst3BusType {
    Main,
    Aux,
    Unknown(i32),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Vst3AudioBusInfo {
    pub index: i32,
    pub direction: Vst3BusDirection,
    pub channel_count: i32,
    pub bus_type: Vst3BusType,
    pub default_active: bool,
    pub control_voltage: bool,
    pub name: Option<String>,
}

impl Vst3AudioBusInfo {
    pub(crate) fn from_abi(index: i32, direction: Vst3BusDirection, info: BusInfo) -> Self {
        Self {
            index,
            direction,
            channel_count: info.channel_count,
            bus_type: bus_type(info.bus_type),
            default_active: (info.flags & VST3_BUS_FLAG_DEFAULT_ACTIVE) != 0,
            control_voltage: (info.flags & VST3_BUS_FLAG_CONTROL_VOLTAGE) != 0,
            name: string128(&info.name),
        }
    }
}

fn bus_type(value: i32) -> Vst3BusType {
    match value {
        VST3_BUS_TYPE_MAIN => Vst3BusType::Main,
        VST3_BUS_TYPE_AUX => Vst3BusType::Aux,
        other => Vst3BusType::Unknown(other),
    }
}

fn string128(value: &[u16; 128]) -> Option<String> {
    let end = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    if end == 0 {
        return None;
    }

    Some(String::from_utf16_lossy(&value[..end]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vst3_abi::VST3_MEDIA_TYPE_AUDIO;

    #[test]
    fn converts_vst3_bus_info_to_safe_type() {
        let mut name = [0; 128];
        for (index, unit) in "Main Out".encode_utf16().enumerate() {
            name[index] = unit;
        }
        let info = BusInfo {
            media_type: VST3_MEDIA_TYPE_AUDIO,
            direction: VST3_BUS_DIRECTION_OUTPUT,
            channel_count: 2,
            name,
            bus_type: VST3_BUS_TYPE_MAIN,
            flags: VST3_BUS_FLAG_DEFAULT_ACTIVE,
        };

        let bus = Vst3AudioBusInfo::from_abi(0, Vst3BusDirection::Output, info);

        assert_eq!(bus.channel_count, 2);
        assert_eq!(bus.bus_type, Vst3BusType::Main);
        assert!(bus.default_active);
        assert_eq!(bus.name.as_deref(), Some("Main Out"));
    }
}
