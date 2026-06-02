pub mod audio_processor;
pub mod bus_info;
pub mod component;
pub mod component_handler;
pub mod connection_point;
pub mod edit_controller;
pub mod error;
pub mod event_list;
pub mod factory;
pub mod host_attributes;
pub mod host_context;
pub mod host_message;
pub mod lifecycle;
pub mod metadata;
pub mod midi_mapping;
pub mod module;
pub mod parameter_changes;
pub mod process_buffers;
pub mod processor;
pub mod state_stream;
pub mod unit_data;
pub mod unit_info;
mod vst3_abi;
mod vst3_bus_abi;
mod vst3_event_abi;

pub use audio_processor::Vst3AudioProcessor;
pub use bus_info::{Vst3AudioBusInfo, Vst3BusDirection, Vst3BusType};
pub use component::Vst3ComponentInstance;
pub use connection_point::Vst3ConnectionPoint;
pub use edit_controller::{Vst3EditController, Vst3ParameterFlags, Vst3ParameterInfo};
pub use error::{HostError, HostResult};
pub use event_list::{
    DEFAULT_MAX_VST3_EVENTS_PER_BLOCK, Vst3InputEvent, Vst3NoteEvent, Vst3PolyPressureEvent,
};
pub use factory::{
    Vst3ComponentProbe, Vst3FactoryClass, Vst3FactoryInfo, Vst3LoadedComponent,
    create_vst3_component_instance, create_vst3_component_probe, load_vst3_factory_info,
};
pub use host_attributes::Vst3HostAttributeList;
pub use host_context::Vst3HostContext;
pub use host_message::Vst3HostMessage;
pub use lifecycle::{Vst3Lifecycle, Vst3LifecycleState, Vst3ProcessingConfig};
pub use metadata::HeadlessPluginMetadata;
pub use midi_mapping::Vst3MidiMapping;
pub use module::{Vst3ModuleProbe, Vst3ModuleSymbols, find_vst3_executable, probe_vst3_module};
pub use parameter_changes::{
    DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK, Vst3ParameterChange, Vst3ParameterChanges,
};
pub use process_buffers::Vst3ProcessBuffers;
pub use processor::{HeadlessPluginInstance, ProcessStats};
pub use unit_data::{Vst3ProgramListData, Vst3UnitData};
pub use unit_info::{
    Vst3ProgramInfo, Vst3ProgramList, Vst3UnitInfo, Vst3UnitInfoEntry, Vst3UnitMetadata,
};
pub use vst3_abi::{VST3_MIDI_CONTROLLER_AFTERTOUCH, VST3_MIDI_CONTROLLER_PITCH_BEND};
