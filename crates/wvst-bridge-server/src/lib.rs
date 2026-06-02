pub mod audio_stream_tracker;
pub mod config;
pub mod control;
pub mod error;
pub mod events;
pub mod host_worker;
pub mod instance_registry;
pub mod metrics;
pub mod plugin_registry;
pub mod server;
pub mod worker_supervisor;

pub use config::BridgeConfig;
pub use error::{BridgeError, BridgeResult};
pub use events::{BridgeEvent, BridgeEventBus, BridgeEventKind};
pub use host_worker::HostWorkerClient;
pub use server::BridgeServer;
