// CS-30003: Reliable Data Transfer over UDP
// Author: Soumyadeb Mukherjee
// Component: Protocol & ARQ - Module Root & Universal Architecture

pub mod sw;
pub mod gbn;
pub mod sr;

pub use sw::{StopAndWaitSender, StopAndWaitReceiver};
pub use gbn::{GoBackNSender, GoBackNReceiver};
pub use sr::{SelectiveRepeatSender, SelectiveRepeatReceiver};

/// The protocol types selectable at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolType {
    StopAndWait,
    GoBackN,
    SelectiveRepeat,
}

impl std::str::FromStr for ProtocolType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace("-", "").replace("_", "").as_str() {
            "stopandwait" | "sw" => Ok(ProtocolType::StopAndWait),
            "gobackn" | "gbn" => Ok(ProtocolType::GoBackN),
            "selectiverepeat" | "sr" => Ok(ProtocolType::SelectiveRepeat),
            other => Err(format!("unknown ARQ protocol: '{}'. Valid options: StopAndWait, GoBackN, SelectiveRepeat", other)),
        }
    }
}

impl std::fmt::Display for ProtocolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolType::StopAndWait => write!(f, "StopAndWait"),
            ProtocolType::GoBackN => write!(f, "GoBackN"),
            ProtocolType::SelectiveRepeat => write!(f, "SelectiveRepeat"),
        }
    }
}
