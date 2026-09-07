use wvst_vst3_host::{
    Vst3BusDirection, Vst3InputEvent, Vst3LifecycleState, Vst3ParameterChange, Vst3ParameterInfo,
    Vst3ProcessOutput, Vst3UnitMetadata,
};

use super::{ConnectionNotifyTarget, WorkerBackend, WorkerBackendError, notify_connection_point};
use crate::ipc::ipc_connection::DecodedMessageAttribute;

#[cfg(test)]
use super::process_test_audio_runtime;

impl WorkerBackend {
    pub(in crate::ipc) fn parameters(&self) -> Result<Vec<Vst3ParameterInfo>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .parameters()
                .map_err(|error| WorkerBackendError::vst3_control("controller.parameters", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(Vec::new()),
        }
    }

    pub(in crate::ipc) fn unit_metadata(
        &self,
    ) -> Result<Option<Vst3UnitMetadata>, WorkerBackendError> {
        match self {
            // A failed/absent optional-interface probe is already exposed in capabilities.
            Self::Vst3Runtime(runtime) if !runtime.capabilities.units => Ok(None),
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .map(|controller| {
                    controller.unit_info().and_then(|unit_info| {
                        unit_info.map(|unit_info| unit_info.metadata()).transpose()
                    })
                })
                .transpose()
                .map(|value| value.flatten())
                .map_err(|error| WorkerBackendError::vst3_control("controller.unit-info", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(None),
        }
    }

    pub(in crate::ipc) fn get_param_normalized(&self, id: u32) -> Option<f64> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .map(|controller| controller.get_param_normalized(id)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => None,
        }
    }

    pub(in crate::ipc) fn param_string_by_value(
        &self,
        id: u32,
        value_normalized: f64,
    ) -> Result<Option<String>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller
                        .param_string_by_value(id, value_normalized)
                        .map_err(|error| {
                            WorkerBackendError::vst3_control(
                                "controller.get-param-string-by-value",
                                error,
                            )
                        })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(in crate::ipc) fn param_value_by_string(
        &self,
        id: u32,
        value: &str,
    ) -> Result<f64, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller
                        .param_value_by_string(id, value)
                        .map_err(|error| {
                            WorkerBackendError::vst3_control(
                                "controller.get-param-value-by-string",
                                error,
                            )
                        })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(in crate::ipc) fn normalized_param_to_plain(
        &self,
        id: u32,
        value_normalized: f64,
    ) -> Option<f64> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .map(|controller| controller.normalized_param_to_plain(id, value_normalized)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => None,
        }
    }

    pub(in crate::ipc) fn plain_param_to_normalized(
        &self,
        id: u32,
        plain_value: f64,
    ) -> Option<f64> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .map(|controller| controller.plain_param_to_normalized(id, plain_value)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => None,
        }
    }

    pub(in crate::ipc) fn set_param_normalized(
        &self,
        id: u32,
        value: f64,
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.set_param_normalized(id, value).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.set-param-normalized", error)
                    })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(in crate::ipc) fn begin_param_edit(&mut self, id: u32) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller_mut()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.begin_edit(id).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.begin-edit", error)
                    })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(in crate::ipc) fn perform_param_edit(
        &mut self,
        id: u32,
        value: f64,
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller_mut()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.perform_edit(id, value).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.perform-edit", error)
                    })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(in crate::ipc) fn end_param_edit(&mut self, id: u32) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller_mut()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.end_edit(id).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.end-edit", error)
                    })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(in crate::ipc) fn controller_state(&self) -> Result<Option<Vec<u8>>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller_state()
                .map_err(|error| WorkerBackendError::vst3_control("controller.get-state", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(None),
        }
    }

    pub(in crate::ipc) fn component_state(&self) -> Result<Option<Vec<u8>>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .component_state()
                .map(Some)
                .map_err(|error| WorkerBackendError::vst3_control("component.get-state", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(None),
        }
    }

    pub(in crate::ipc) fn set_controller_state(
        &self,
        state: &[u8],
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .controller()
                .ok_or_else(|| WorkerBackendError::plain("edit controller not available"))
                .and_then(|controller| {
                    controller.set_state(state).map_err(|error| {
                        WorkerBackendError::vst3_control("controller.set-state", error)
                    })
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("edit controller not available"))
            }
        }
    }

    pub(in crate::ipc) fn set_component_state(
        &mut self,
        state: &[u8],
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_component_state(state)
                .map_err(|error| WorkerBackendError::vst3_control("component.set-state", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("component state not available"))
            }
        }
    }

    pub(in crate::ipc) fn notify_component(
        &mut self,
        message_id: &str,
        attributes: &[DecodedMessageAttribute],
    ) -> Result<Option<()>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => notify_connection_point(
                &mut runtime.component,
                ConnectionNotifyTarget::Component,
                message_id,
                attributes,
            ),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(None),
        }
    }

    pub(in crate::ipc) fn notify_controller(
        &mut self,
        message_id: &str,
        attributes: &[DecodedMessageAttribute],
    ) -> Result<Option<()>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => notify_connection_point(
                &mut runtime.component,
                ConnectionNotifyTarget::Controller,
                message_id,
                attributes,
            ),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(None),
        }
    }

    pub(in crate::ipc) fn select_unit(&self, unit_id: i32) -> Result<i32, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .select_unit(unit_id)
                .map_err(|error| WorkerBackendError::vst3_control("controller.select-unit", error))?
                .ok_or_else(|| WorkerBackendError::plain("unit info not available")),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Err(WorkerBackendError::plain("unit info not available")),
        }
    }

    pub(in crate::ipc) fn unit_by_audio_bus(
        &self,
        direction: Vst3BusDirection,
        bus_index: i32,
        channel: i32,
    ) -> Result<Option<i32>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .unit_by_audio_bus(direction, bus_index, channel)
                .map_err(|error| WorkerBackendError::vst3_control("controller.unit-by-bus", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(None),
        }
    }

    pub(in crate::ipc) fn set_unit_program_data(
        &self,
        list_or_unit_id: i32,
        program_index: i32,
        data: &[u8],
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_unit_program_data(list_or_unit_id, program_index, data)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("controller.set-unit-program-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("unit info not available")),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Err(WorkerBackendError::plain("unit info not available")),
        }
    }

    pub(in crate::ipc) fn program_data_supported(
        &self,
        list_id: i32,
    ) -> Result<bool, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .program_data_supported(list_id)
                .map(|supported| supported.unwrap_or(false))
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.program-data-supported", error)
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(false),
        }
    }

    pub(in crate::ipc) fn get_program_data(
        &self,
        list_id: i32,
        program_index: i32,
    ) -> Result<Vec<u8>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .get_program_data(list_id, program_index)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.get-program-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("program list data not available")),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("program list data not available"))
            }
        }
    }

    pub(in crate::ipc) fn set_program_data(
        &self,
        list_id: i32,
        program_index: i32,
        data: &[u8],
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_program_data(list_id, program_index, data)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.set-program-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("program list data not available")),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => {
                Err(WorkerBackendError::plain("program list data not available"))
            }
        }
    }

    pub(in crate::ipc) fn unit_data_supported(
        &self,
        unit_id: i32,
    ) -> Result<bool, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .unit_data_supported(unit_id)
                .map(|supported| supported.unwrap_or(false))
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.unit-data-supported", error)
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(false),
        }
    }

    pub(in crate::ipc) fn get_unit_data(
        &self,
        unit_id: i32,
    ) -> Result<Vec<u8>, WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .get_unit_data(unit_id)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.get-unit-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("unit data not available")),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Err(WorkerBackendError::plain("unit data not available")),
        }
    }

    pub(in crate::ipc) fn set_unit_data(
        &self,
        unit_id: i32,
        data: &[u8],
    ) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .set_unit_data(unit_id, data)
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.set-unit-data", error)
                })?
                .ok_or_else(|| WorkerBackendError::plain("unit data not available")),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Err(WorkerBackendError::plain("unit data not available")),
        }
    }

    pub(in crate::ipc) fn start_processing(&mut self) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .instance_mut()
                .start_processing()
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.start-processing", error)
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(()),
        }
    }

    pub(in crate::ipc) fn stop_processing(&mut self) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .instance_mut()
                .stop_processing()
                .map_err(|error| {
                    WorkerBackendError::vst3_control("component.stop-processing", error)
                }),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(()),
        }
    }

    pub(in crate::ipc) fn process_interleaved_f32(
        &mut self,
        frames: usize,
        input: &[f32],
        events: &[Vst3InputEvent],
        parameter_changes: &[Vst3ParameterChange],
        output: &mut [f32],
        process_output: &mut Vst3ProcessOutput,
    ) -> Result<(), WorkerBackendError> {
        process_output.clear();
        match self {
            Self::Vst3Runtime(runtime) => runtime
                .component
                .instance_mut()
                .process_interleaved_f32_with_io_events_and_parameters(
                    frames,
                    input,
                    events,
                    parameter_changes,
                    output,
                    process_output,
                )
                .map_err(|error| WorkerBackendError::vst3_process("component.process", error)),
            #[cfg(test)]
            Self::TestAudioRuntime(runtime) => {
                process_test_audio_runtime(runtime, frames, input, output)
            }
        }
    }

    pub(in crate::ipc) fn midi_controller_param_id(
        &self,
        channel: u8,
        controller: i16,
    ) -> Option<u32> {
        match self {
            Self::Vst3Runtime(runtime) => runtime.midi_mapping.get(channel, controller),
            #[cfg(test)]
            Self::TestAudioRuntime(_) => None,
        }
    }

    pub(in crate::ipc) fn terminate(&mut self, processing: bool) -> Result<(), WorkerBackendError> {
        match self {
            Self::Vst3Runtime(runtime) => {
                if processing {
                    runtime
                        .component
                        .instance_mut()
                        .stop_processing()
                        .map_err(|error| {
                            WorkerBackendError::vst3_control("component.stop-processing", error)
                        })?;
                }
                if runtime.component.instance().state() != Vst3LifecycleState::Terminated {
                    runtime
                        .component
                        .instance_mut()
                        .terminate()
                        .map_err(|error| {
                            WorkerBackendError::vst3_control("component.terminate", error)
                        })?;
                }
                runtime.component.terminate_controller().map_err(|error| {
                    WorkerBackendError::vst3_control("controller.terminate", error)
                })?;
                Ok(())
            }
            #[cfg(test)]
            Self::TestAudioRuntime(_) => Ok(()),
        }
    }
}
