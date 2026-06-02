use std::ffi::c_void;
use std::ptr::NonNull;

use serde::{Deserialize, Serialize};

use crate::component_handler::Vst3ComponentHandler;
use crate::midi_mapping::Vst3MidiMapping;
use crate::state_stream::Vst3StateStream;
use crate::unit_info::Vst3UnitInfo;
use crate::vst3_abi::{
    IEditController, IEditControllerVTable, K_RESULT_FALSE, K_RESULT_OK, ParamId, ParamValue,
    ParameterInfo as RawParameterInfo, String128, VST3_I_MIDI_MAPPING_IID, VST3_I_UNIT_INFO_IID,
    VST3_PARAMETER_CAN_AUTOMATE, VST3_PARAMETER_IS_BYPASS, VST3_PARAMETER_IS_HIDDEN,
    VST3_PARAMETER_IS_LIST, VST3_PARAMETER_IS_PROGRAM_CHANGE, VST3_PARAMETER_IS_READ_ONLY,
    VST3_PARAMETER_IS_WRAP_AROUND, parse_tuid_hex,
};
use crate::{HostError, HostResult, Vst3HostContext};

#[derive(Debug)]
pub struct Vst3EditController {
    controller: NonNull<IEditController>,
    host_context: Vst3HostContext,
    component_handler: Vst3ComponentHandler,
    initialized: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3ParameterInfo {
    pub id: u32,
    pub title: Option<String>,
    pub short_title: Option<String>,
    pub units: Option<String>,
    pub step_count: i32,
    pub default_normalized_value: f64,
    pub unit_id: i32,
    pub flags: Vst3ParameterFlags,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3ParameterFlags {
    pub raw: i32,
    pub can_automate: bool,
    pub read_only: bool,
    pub wrap_around: bool,
    pub list: bool,
    pub hidden: bool,
    pub program_change: bool,
    pub bypass: bool,
}

impl Vst3EditController {
    /// # Safety
    ///
    /// `controller` must be an owned, valid VST3 `IEditController` pointer.
    /// This holder releases that reference on drop.
    pub unsafe fn from_raw(controller: *mut c_void) -> HostResult<Self> {
        let controller = NonNull::new(controller.cast::<IEditController>())
            .ok_or(HostError::EditControllerReturnedNull)?;

        // SAFETY: The caller guarantees the pointer is a valid IEditController
        // object for the lifetime transferred to this holder.
        let vtable = unsafe { controller.as_ref().vtable };
        if vtable.is_null() {
            return Err(HostError::EditControllerVTableMissing);
        }

        Ok(Self {
            controller,
            host_context: Vst3HostContext::new("WVST"),
            component_handler: Vst3ComponentHandler::new(),
            initialized: false,
        })
    }

    pub fn initialize(&mut self) -> HostResult<()> {
        if self.initialized {
            return Ok(());
        }

        let context = self.host_context.as_funknown_ptr();
        self.call_result("initialize", |controller, vtable| unsafe {
            // SAFETY: controller/vtable were validated by from_raw. The host
            // context object is owned by this controller holder.
            (vtable.initialize)(controller, context)
        })?;
        self.set_component_handler()?;
        self.initialized = true;
        Ok(())
    }

    pub fn terminate(&mut self) -> HostResult<()> {
        if !self.initialized {
            return Ok(());
        }

        self.call_result("terminate", |controller, vtable| unsafe {
            // SAFETY: controller/vtable were validated by from_raw.
            (vtable.terminate)(controller)
        })?;
        self.initialized = false;
        Ok(())
    }

    pub fn parameter_count(&self) -> HostResult<i32> {
        let count = unsafe {
            // SAFETY: controller/vtable were validated by from_raw.
            (self.vtable().get_parameter_count)(self.controller.as_ptr())
        };
        if count >= 0 {
            Ok(count)
        } else {
            Err(HostError::EditControllerCallFailed {
                method: "getParameterCount",
                result: count,
            })
        }
    }

    pub fn parameter_info(&self, index: i32) -> HostResult<Vst3ParameterInfo> {
        let mut info = RawParameterInfo::default();
        self.call_result("getParameterInfo", |controller, vtable| unsafe {
            // SAFETY: info is writable stack storage matching VST3 ABI.
            (vtable.get_parameter_info)(controller, index, &mut info)
        })?;
        Ok(Vst3ParameterInfo::from_abi(info))
    }

    pub fn parameters(&self) -> HostResult<Vec<Vst3ParameterInfo>> {
        let count = self.parameter_count()?;
        let mut parameters = Vec::with_capacity(count as usize);
        for index in 0..count {
            parameters.push(self.parameter_info(index)?);
        }
        Ok(parameters)
    }

    pub fn get_param_normalized(&self, id: ParamId) -> ParamValue {
        unsafe {
            // SAFETY: controller/vtable were validated by from_raw.
            (self.vtable().get_param_normalized)(self.controller.as_ptr(), id)
        }
    }

    pub fn set_param_normalized(&self, id: ParamId, value: ParamValue) -> HostResult<()> {
        self.call_result("setParamNormalized", |controller, vtable| unsafe {
            // SAFETY: controller/vtable were validated by from_raw.
            (vtable.set_param_normalized)(controller, id, value)
        })
    }

    pub fn param_string_by_value(
        &self,
        id: ParamId,
        value_normalized: ParamValue,
    ) -> HostResult<Option<String>> {
        let mut string = [0; 128];
        self.call_result("getParamStringByValue", |controller, vtable| unsafe {
            // SAFETY: string is writable stack storage matching String128 ABI.
            (vtable.get_param_string_by_value)(controller, id, value_normalized, &mut string)
        })?;
        Ok(string128_to_string(&string))
    }

    pub fn param_value_by_string(&self, id: ParamId, value: &str) -> HostResult<ParamValue> {
        let mut input = string128(value);
        let mut normalized = 0.0;
        self.call_result("getParamValueByString", |controller, vtable| unsafe {
            // SAFETY: input is UTF-16 String128 storage and normalized points to
            // writable stack storage.
            (vtable.get_param_value_by_string)(controller, id, input.as_mut_ptr(), &mut normalized)
        })?;
        Ok(normalized)
    }

    pub fn normalized_param_to_plain(
        &self,
        id: ParamId,
        value_normalized: ParamValue,
    ) -> ParamValue {
        unsafe {
            // SAFETY: controller/vtable were validated by from_raw.
            (self.vtable().normalized_param_to_plain)(
                self.controller.as_ptr(),
                id,
                value_normalized,
            )
        }
    }

    pub fn plain_param_to_normalized(&self, id: ParamId, plain_value: ParamValue) -> ParamValue {
        unsafe {
            // SAFETY: controller/vtable were validated by from_raw.
            (self.vtable().plain_param_to_normalized)(self.controller.as_ptr(), id, plain_value)
        }
    }

    pub fn get_state(&self) -> HostResult<Vec<u8>> {
        let mut stream = Vst3StateStream::writable();
        self.call_result("getState", |controller, vtable| unsafe {
            // SAFETY: stream object remains live for the duration of the call.
            (vtable.get_state)(controller, stream.as_mut_ptr())
        })?;
        Ok(stream.into_bytes())
    }

    pub fn set_state(&self, state: &[u8]) -> HostResult<()> {
        let mut stream = Vst3StateStream::from_bytes(state.to_vec());
        self.call_result("setState", |controller, vtable| unsafe {
            // SAFETY: stream object remains live for the duration of the call.
            (vtable.set_state)(controller, stream.as_mut_ptr())
        })
    }

    pub fn set_component_state(&self, state: &[u8]) -> HostResult<()> {
        let mut stream = Vst3StateStream::from_bytes(state.to_vec());
        self.call_result("setComponentState", |controller, vtable| unsafe {
            // SAFETY: stream object remains live for the duration of the call.
            (vtable.set_component_state)(controller, stream.as_mut_ptr())
        })
    }

    pub fn midi_mapping(&self) -> HostResult<Option<Vst3MidiMapping>> {
        let Some(object) = self.query_optional_interface(VST3_I_MIDI_MAPPING_IID)? else {
            return Ok(None);
        };
        // SAFETY: queryInterface returned a referenced IMidiMapping pointer.
        unsafe { Vst3MidiMapping::from_raw(object.cast()) }.map(Some)
    }

    pub fn unit_info(&self) -> HostResult<Option<Vst3UnitInfo>> {
        let Some(object) = self.query_optional_interface(VST3_I_UNIT_INFO_IID)? else {
            return Ok(None);
        };
        // SAFETY: queryInterface returned a referenced IUnitInfo pointer.
        unsafe { Vst3UnitInfo::from_raw(object.cast()) }.map(Some)
    }

    fn set_component_handler(&mut self) -> HostResult<()> {
        let handler = self.component_handler.as_mut_ptr();
        self.call_result("setComponentHandler", |controller, vtable| unsafe {
            // SAFETY: handler is owned by this holder and remains live until
            // after controller termination/drop.
            (vtable.set_component_handler)(controller, handler)
        })
    }

    fn call_result(
        &self,
        method: &'static str,
        call: impl FnOnce(*mut IEditController, &IEditControllerVTable) -> i32,
    ) -> HostResult<()> {
        let result = call(self.controller.as_ptr(), self.vtable());
        if result == K_RESULT_OK {
            Ok(())
        } else {
            Err(HostError::EditControllerCallFailed { method, result })
        }
    }

    fn vtable(&self) -> &IEditControllerVTable {
        // SAFETY: from_raw validated both the object pointer and vtable pointer.
        unsafe { &*self.controller.as_ref().vtable }
    }

    fn query_optional_interface(&self, interface_id: &str) -> HostResult<Option<*mut c_void>> {
        let interface_tuid = parse_tuid_hex(interface_id)
            .ok_or_else(|| HostError::InvalidInterfaceId(interface_id.to_string()))?;
        let mut object: *mut c_void = std::ptr::null_mut();
        let result = unsafe {
            // SAFETY: controller/vtable were validated by from_raw. object is
            // writable stack storage for the optional queried interface.
            (self.vtable().query_interface)(
                self.controller.as_ptr(),
                interface_tuid.as_ptr().cast(),
                &mut object,
            )
        };

        if result == K_RESULT_FALSE {
            return Ok(None);
        }
        if result != K_RESULT_OK {
            return Err(HostError::InterfaceQueryFailed {
                interface_id: interface_id.to_string(),
                result,
            });
        }
        if object.is_null() {
            return Err(HostError::InterfaceReturnedNull {
                interface_id: interface_id.to_string(),
            });
        }

        Ok(Some(object))
    }
}

impl Drop for Vst3EditController {
    fn drop(&mut self) {
        let _ = self.terminate();
        let controller = self.controller.as_ptr();
        let vtable = self.vtable();

        // SAFETY: from_raw transfers one owned IEditController reference to
        // this holder; Drop releases that reference exactly once.
        unsafe {
            (vtable.release)(controller);
        }
    }
}

impl Vst3ParameterInfo {
    fn from_abi(info: RawParameterInfo) -> Self {
        Self {
            id: info.id,
            title: string128_to_string(&info.title),
            short_title: string128_to_string(&info.short_title),
            units: string128_to_string(&info.units),
            step_count: info.step_count,
            default_normalized_value: info.default_normalized_value,
            unit_id: info.unit_id,
            flags: Vst3ParameterFlags::from_bits(info.flags),
        }
    }
}

impl Vst3ParameterFlags {
    fn from_bits(raw: i32) -> Self {
        Self {
            raw,
            can_automate: raw & VST3_PARAMETER_CAN_AUTOMATE != 0,
            read_only: raw & VST3_PARAMETER_IS_READ_ONLY != 0,
            wrap_around: raw & VST3_PARAMETER_IS_WRAP_AROUND != 0,
            list: raw & VST3_PARAMETER_IS_LIST != 0,
            hidden: raw & VST3_PARAMETER_IS_HIDDEN != 0,
            program_change: raw & VST3_PARAMETER_IS_PROGRAM_CHANGE != 0,
            bypass: raw & VST3_PARAMETER_IS_BYPASS != 0,
        }
    }
}

fn string128(value: &str) -> String128 {
    let mut output = [0; 128];
    for (index, unit) in value.encode_utf16().take(127).enumerate() {
        output[index] = unit;
    }
    output
}

pub(crate) fn string128_to_string(value: &String128) -> Option<String> {
    let len = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    let text = String::from_utf16_lossy(&value[..len]).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(test)]
#[path = "edit_controller_tests.rs"]
mod tests;
