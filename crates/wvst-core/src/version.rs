#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

impl ProtocolVersion {
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    pub const fn is_compatible_with(self, other: Self) -> bool {
        self.major == other.major
    }
}

pub const CURRENT_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion::new(1, 0);
