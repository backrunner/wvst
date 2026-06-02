pub const VST3_BUS_TYPE_MAIN: i32 = 0;
pub const VST3_BUS_TYPE_AUX: i32 = 1;
pub const VST3_BUS_FLAG_DEFAULT_ACTIVE: u32 = 1 << 0;
pub const VST3_BUS_FLAG_CONTROL_VOLTAGE: u32 = 1 << 1;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BusInfo {
    pub media_type: i32,
    pub direction: i32,
    pub channel_count: i32,
    pub name: [u16; 128],
    pub bus_type: i32,
    pub flags: u32,
}

impl Default for BusInfo {
    fn default() -> Self {
        Self {
            media_type: 0,
            direction: 0,
            channel_count: 0,
            name: [0; 128],
            bus_type: VST3_BUS_TYPE_MAIN,
            flags: 0,
        }
    }
}
