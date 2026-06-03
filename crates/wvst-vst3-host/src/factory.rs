use serde::{Deserialize, Serialize};

use crate::{
    HostResult, Vst3BusDirection, Vst3ComponentHandlerSnapshot, Vst3ComponentInstance,
    Vst3ConnectionPoint, Vst3EditController, Vst3HostMessage, Vst3ParameterInfo,
    Vst3ProcessingConfig,
};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3FactoryInfo {
    pub vendor: Option<String>,
    pub url: Option<String>,
    pub email: Option<String>,
    pub flags: i32,
    pub classes: Vec<Vst3FactoryClass>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3FactoryClass {
    pub class_id: String,
    pub cardinality: i32,
    pub category: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3ComponentProbe {
    pub bundle_path: String,
    pub class_id: String,
    pub interface_id: String,
    pub audio_processor: bool,
    pub created: bool,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3ControllerComponentStateSync {
    pub attempted: bool,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component_state_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub struct Vst3LoadedComponent {
    instance: Vst3ComponentInstance,
    controller: Option<Vst3EditController>,
    controller_component_state_sync: Option<Vst3ControllerComponentStateSync>,
    connection_points: Option<Vst3ConnectedPair>,
    _module: Option<platform::LoadedPluginModule>,
}

impl Vst3LoadedComponent {
    fn new(
        instance: Vst3ComponentInstance,
        controller: Option<Vst3EditController>,
        module: platform::LoadedPluginModule,
    ) -> Self {
        Self {
            instance,
            controller,
            controller_component_state_sync: None,
            connection_points: None,
            _module: Some(module),
        }
    }

    #[cfg(test)]
    fn new_for_test(
        instance: Vst3ComponentInstance,
        controller: Option<Vst3EditController>,
    ) -> Self {
        Self {
            instance,
            controller,
            controller_component_state_sync: None,
            connection_points: None,
            _module: None,
        }
    }

    pub fn instance(&self) -> &Vst3ComponentInstance {
        &self.instance
    }

    pub fn instance_mut(&mut self) -> &mut Vst3ComponentInstance {
        &mut self.instance
    }

    pub fn controller(&self) -> Option<&Vst3EditController> {
        self.controller.as_ref()
    }

    pub fn controller_mut(&mut self) -> Option<&mut Vst3EditController> {
        self.controller.as_mut()
    }

    pub fn initialize_controller(&mut self) -> HostResult<()> {
        if self.controller.is_some() {
            let controller = self.controller.as_mut().expect("controller checked");
            controller.initialize()?;
            self.controller_component_state_sync = Some(self.sync_controller_component_state());
            self.connect_component_controller()?;
        }
        Ok(())
    }

    pub fn terminate_controller(&mut self) -> HostResult<()> {
        self.disconnect_component_controller()?;
        if let Some(controller) = self.controller.as_mut() {
            controller.terminate()?;
        }
        Ok(())
    }

    pub fn parameters(&self) -> HostResult<Vec<Vst3ParameterInfo>> {
        self.controller
            .as_ref()
            .map_or_else(|| Ok(Vec::new()), Vst3EditController::parameters)
    }

    pub fn component_state(&self) -> HostResult<Vec<u8>> {
        self.instance.get_state()
    }

    pub fn controller_state(&self) -> HostResult<Option<Vec<u8>>> {
        self.controller
            .as_ref()
            .map(|controller| controller.get_state())
            .transpose()
    }

    pub fn component_handler_snapshot(&self) -> Option<Vst3ComponentHandlerSnapshot> {
        self.controller
            .as_ref()
            .map(Vst3EditController::component_handler_snapshot)
    }

    pub fn connection_points_connected(&self) -> bool {
        self.connection_points.is_some()
    }

    pub fn controller_component_state_sync(&self) -> Option<&Vst3ControllerComponentStateSync> {
        self.controller_component_state_sync.as_ref()
    }

    pub fn notify_component(&self, message: &mut Vst3HostMessage) -> HostResult<Option<()>> {
        let Some(connection_points) = self.connection_points.as_ref() else {
            return Ok(None);
        };
        connection_points.component.notify(message)?;
        Ok(Some(()))
    }

    pub fn notify_controller(&self, message: &mut Vst3HostMessage) -> HostResult<Option<()>> {
        let Some(connection_points) = self.connection_points.as_ref() else {
            return Ok(None);
        };
        connection_points.controller.notify(message)?;
        Ok(Some(()))
    }

    pub fn set_component_state(&mut self, state: &[u8]) -> HostResult<()> {
        self.instance.set_state(state)?;
        self.apply_controller_component_state(state)
    }

    pub fn set_controller_state(&mut self, state: &[u8]) -> HostResult<()> {
        if let Some(controller) = self.controller.as_ref() {
            controller.set_state(state)?;
        }
        Ok(())
    }

    fn sync_controller_component_state(&mut self) -> Vst3ControllerComponentStateSync {
        let state = match self.instance.get_state() {
            Ok(state) => state,
            Err(error) => {
                return Vst3ControllerComponentStateSync {
                    attempted: true,
                    error: Some(error.to_string()),
                    ..Vst3ControllerComponentStateSync::default()
                };
            }
        };
        let _ = self.apply_controller_component_state(&state);
        self.controller_component_state_sync
            .clone()
            .unwrap_or_default()
    }

    fn apply_controller_component_state(&mut self, state: &[u8]) -> HostResult<()> {
        let component_state_bytes = Some(state.len());
        let Some(controller) = self.controller.as_ref() else {
            self.controller_component_state_sync = None;
            return Ok(());
        };
        match controller.set_component_state(state) {
            Ok(()) => {
                self.controller_component_state_sync = Some(Vst3ControllerComponentStateSync {
                    attempted: true,
                    success: true,
                    component_state_bytes,
                    error: None,
                });
                Ok(())
            }
            Err(error) => {
                self.controller_component_state_sync = Some(Vst3ControllerComponentStateSync {
                    attempted: true,
                    success: false,
                    component_state_bytes,
                    error: Some(error.to_string()),
                });
                Err(error)
            }
        }
    }

    pub fn select_unit(&self, unit_id: i32) -> HostResult<Option<i32>> {
        let Some(controller) = self.controller.as_ref() else {
            return Ok(None);
        };
        let Some(unit_info) = controller.unit_info()? else {
            return Ok(None);
        };
        unit_info.select_unit(unit_id)?;
        Ok(Some(unit_info.selected_unit()))
    }

    pub fn unit_by_audio_bus(
        &self,
        direction: Vst3BusDirection,
        bus_index: i32,
        channel: i32,
    ) -> HostResult<Option<i32>> {
        let Some(controller) = self.controller.as_ref() else {
            return Ok(None);
        };
        let Some(unit_info) = controller.unit_info()? else {
            return Ok(None);
        };
        unit_info.unit_by_audio_bus(direction, bus_index, channel)
    }

    pub fn set_unit_program_data(
        &self,
        list_or_unit_id: i32,
        program_index: i32,
        data: &[u8],
    ) -> HostResult<Option<()>> {
        let Some(controller) = self.controller.as_ref() else {
            return Ok(None);
        };
        let Some(unit_info) = controller.unit_info()? else {
            return Ok(None);
        };
        unit_info.set_unit_program_data(list_or_unit_id, program_index, data)?;
        Ok(Some(()))
    }

    pub fn program_data_supported(&self, list_id: i32) -> HostResult<Option<bool>> {
        self.instance
            .program_list_data()?
            .map(|data| data.program_data_supported(list_id))
            .transpose()
    }

    pub fn has_program_list_data(&self) -> HostResult<bool> {
        self.instance.program_list_data().map(|data| data.is_some())
    }

    pub fn get_program_data(
        &self,
        list_id: i32,
        program_index: i32,
    ) -> HostResult<Option<Vec<u8>>> {
        self.instance
            .program_list_data()?
            .map(|data| data.get_program_data(list_id, program_index))
            .transpose()
    }

    pub fn set_program_data(
        &self,
        list_id: i32,
        program_index: i32,
        data: &[u8],
    ) -> HostResult<Option<()>> {
        self.instance
            .program_list_data()?
            .map(|program_data| program_data.set_program_data(list_id, program_index, data))
            .transpose()
    }

    pub fn unit_data_supported(&self, unit_id: i32) -> HostResult<Option<bool>> {
        self.instance
            .unit_data()?
            .map(|data| data.unit_data_supported(unit_id))
            .transpose()
    }

    pub fn has_unit_data(&self) -> HostResult<bool> {
        self.instance.unit_data().map(|data| data.is_some())
    }

    pub fn get_unit_data(&self, unit_id: i32) -> HostResult<Option<Vec<u8>>> {
        self.instance
            .unit_data()?
            .map(|data| data.get_unit_data(unit_id))
            .transpose()
    }

    pub fn set_unit_data(&self, unit_id: i32, data: &[u8]) -> HostResult<Option<()>> {
        self.instance
            .unit_data()?
            .map(|unit_data| unit_data.set_unit_data(unit_id, data))
            .transpose()
    }

    fn connect_component_controller(&mut self) -> HostResult<()> {
        if self.connection_points.is_some() {
            return Ok(());
        }
        let Some(controller) = self.controller.as_ref() else {
            return Ok(());
        };
        let Some(component_point) = self.instance.connection_point()? else {
            return Ok(());
        };
        let Some(controller_point) = controller.connection_point()? else {
            return Ok(());
        };

        component_point.connect(&controller_point)?;
        if let Err(error) = controller_point.connect(&component_point) {
            let _ = component_point.disconnect(&controller_point);
            return Err(error);
        }
        self.connection_points = Some(Vst3ConnectedPair {
            component: component_point,
            controller: controller_point,
        });
        Ok(())
    }

    fn disconnect_component_controller(&mut self) -> HostResult<()> {
        let Some(connection_points) = self.connection_points.take() else {
            return Ok(());
        };

        let first = connection_points
            .component
            .disconnect(&connection_points.controller);
        let second = connection_points
            .controller
            .disconnect(&connection_points.component);
        first?;
        second
    }
}

#[derive(Debug)]
struct Vst3ConnectedPair {
    component: Vst3ConnectionPoint,
    controller: Vst3ConnectionPoint,
}

// SAFETY: `Vst3LoadedComponent` is an owning runtime holder. Its raw plugin
// pointers are released exactly once by their wrappers, and process-buffer ABI
// pointers target heap allocations owned by the same holder. Moving the holder
// to another thread does not invalidate those allocations. The type is not
// `Sync`; callers still need exclusive mutable access for lifecycle and process
// calls, and WVST workers additionally serialize access with their instance
// mutex.
unsafe impl Send for Vst3LoadedComponent {}

fn factory_create_instance_tuids(
    class_id: &str,
    interface_id: &str,
) -> HostResult<([u8; 16], [u8; 16])> {
    let class_tuid = crate::vst3_abi::parse_tuid_hex(class_id)
        .ok_or_else(|| crate::HostError::InvalidClassId(class_id.to_string()))?;
    let interface_tuid = crate::vst3_abi::parse_tuid_hex(interface_id)
        .ok_or_else(|| crate::HostError::InvalidInterfaceId(interface_id.to_string()))?;

    Ok((class_tuid, interface_tuid))
}

pub fn load_vst3_factory_info(
    bundle_path: impl AsRef<std::path::Path>,
) -> HostResult<Vst3FactoryInfo> {
    platform::load_vst3_factory_info(bundle_path.as_ref())
}

pub fn create_vst3_component_probe(
    bundle_path: impl AsRef<std::path::Path>,
    class_id: &str,
) -> HostResult<Vst3ComponentProbe> {
    let class_id = crate::vst3_abi::normalize_fuid_string(class_id)
        .ok_or_else(|| crate::HostError::InvalidClassId(class_id.to_string()))?;

    platform::create_vst3_component_probe(
        bundle_path.as_ref(),
        &class_id,
        crate::vst3_abi::VST3_I_COMPONENT_IID,
    )
}

pub fn create_vst3_component_instance(
    bundle_path: impl AsRef<std::path::Path>,
    class_id: &str,
    processing_config: Vst3ProcessingConfig,
) -> HostResult<Vst3LoadedComponent> {
    let class_id = crate::vst3_abi::normalize_fuid_string(class_id)
        .ok_or_else(|| crate::HostError::InvalidClassId(class_id.to_string()))?;

    platform::create_vst3_component_instance(bundle_path.as_ref(), &class_id, processing_config)
}

#[cfg(target_os = "macos")]
#[path = "factory/platform_macos.rs"]
mod platform;

#[cfg(not(target_os = "macos"))]
#[path = "factory/platform_stub.rs"]
mod platform;

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
