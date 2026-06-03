use super::super::report::RuntimeProbeStatus;
use super::RuntimeProbeExpectations;

impl RuntimeProbeExpectations {
    pub const fn expected_status(mut self, status: RuntimeProbeStatus) -> Self {
        self.expected_status = Some(status);
        self
    }

    pub const fn require_non_zero_output(mut self) -> Self {
        self.require_non_zero_output = true;
        self
    }

    pub const fn require_silent_output(mut self) -> Self {
        self.require_silent_output = true;
        self
    }

    pub const fn max_non_finite_output_samples(mut self, samples: u64) -> Self {
        self.max_non_finite_output_samples = Some(samples);
        self
    }

    pub const fn max_clipped_output_samples(mut self, samples: u64) -> Self {
        self.max_clipped_output_samples = Some(samples);
        self
    }

    pub const fn max_silent_output_blocks(mut self, blocks: u64) -> Self {
        self.max_silent_output_blocks = Some(blocks);
        self
    }

    pub const fn min_output_rms_milli(mut self, rms_milli: u32) -> Self {
        self.min_output_rms_milli = Some(rms_milli);
        self
    }

    pub const fn max_output_peak_milli(mut self, peak_milli: u32) -> Self {
        self.max_output_peak_milli = Some(peak_milli);
        self
    }

    pub const fn require_note_response(mut self) -> Self {
        self.require_note_response = true;
        self
    }

    pub const fn max_note_to_audio_frames(mut self, frames: u64) -> Self {
        self.max_note_to_audio_frames = Some(frames);
        self
    }

    pub const fn max_note_to_audio_micros(mut self, micros: u64) -> Self {
        self.max_note_to_audio_micros = Some(micros);
        self
    }

    pub const fn min_parameter_count(mut self, count: u64) -> Self {
        self.min_parameter_count = Some(count);
        self
    }

    pub const fn min_automatable_parameters(mut self, count: u64) -> Self {
        self.min_automatable_parameters = Some(count);
        self
    }

    pub const fn require_component_state(mut self) -> Self {
        self.require_component_state = true;
        self
    }

    pub const fn min_component_state_bytes(mut self, bytes: u64) -> Self {
        self.min_component_state_bytes = Some(bytes);
        self
    }

    pub const fn require_component_state_roundtrip(mut self) -> Self {
        self.require_component_state_roundtrip = true;
        self
    }

    pub const fn require_controller_state(mut self) -> Self {
        self.require_controller_state = true;
        self
    }

    pub const fn min_controller_state_bytes(mut self, bytes: u64) -> Self {
        self.min_controller_state_bytes = Some(bytes);
        self
    }

    pub const fn require_controller_state_roundtrip(mut self) -> Self {
        self.require_controller_state_roundtrip = true;
        self
    }

    pub const fn require_controller_component_state_sync(mut self) -> Self {
        self.require_controller_component_state_sync = true;
        self
    }

    pub const fn min_controller_component_state_sync_bytes(mut self, bytes: u64) -> Self {
        self.min_controller_component_state_sync_bytes = Some(bytes);
        self
    }

    pub const fn require_unit_info(mut self) -> Self {
        self.require_unit_info = true;
        self
    }

    pub const fn min_unit_count(mut self, count: u64) -> Self {
        self.min_unit_count = Some(count);
        self
    }

    pub const fn min_program_list_count(mut self, count: u64) -> Self {
        self.min_program_list_count = Some(count);
        self
    }

    pub const fn min_total_programs(mut self, count: u64) -> Self {
        self.min_total_programs = Some(count);
        self
    }

    pub const fn require_program_list_data(mut self) -> Self {
        self.require_program_list_data = true;
        self
    }

    pub const fn min_program_list_data_supported(mut self, count: u64) -> Self {
        self.min_program_list_data_supported = Some(count);
        self
    }

    pub const fn require_unit_data(mut self) -> Self {
        self.require_unit_data = true;
        self
    }

    pub const fn min_unit_data_supported(mut self, count: u64) -> Self {
        self.min_unit_data_supported = Some(count);
        self
    }

    pub const fn require_component_handler(mut self) -> Self {
        self.require_component_handler = true;
        self
    }

    pub const fn require_component_handler_edit_probe(mut self) -> Self {
        self.require_component_handler_edit_probe = true;
        self
    }

    pub const fn require_connection_points(mut self) -> Self {
        self.require_connection_points = true;
        self
    }

    pub const fn require_connection_notify_probe(mut self) -> Self {
        self.require_connection_notify_probe = true;
        self
    }

    pub const fn min_component_handler_events(mut self, count: u64) -> Self {
        self.min_component_handler_events = Some(count);
        self
    }

    pub const fn require_no_input_bus(mut self) -> Self {
        self.require_no_input_bus = true;
        self
    }

    pub const fn expected_input_bus_index(mut self, index: i32) -> Self {
        self.expected_input_bus_index = Some(index);
        self
    }

    pub const fn expected_output_bus_index(mut self, index: i32) -> Self {
        self.expected_output_bus_index = Some(index);
        self
    }

    pub const fn expected_input_bus_channels(mut self, channels: u16) -> Self {
        self.expected_input_bus_channels = Some(channels);
        self
    }

    pub const fn expected_output_bus_channels(mut self, channels: u16) -> Self {
        self.expected_output_bus_channels = Some(channels);
        self
    }

    pub fn expected_compatibility_category(mut self, category: impl Into<String>) -> Self {
        self.expected_compatibility_category = Some(category.into());
        self
    }

    pub fn expected_classification_category(mut self, category: impl Into<String>) -> Self {
        self.expected_classification_category = Some(category.into());
        self
    }
}
