use std::ffi::c_void;
use std::ptr::NonNull;

use crate::vst3_abi::{
    BusInfo, FUnknown, IComponent, IComponentVTable, K_NOT_IMPLEMENTED, K_RESULT_FALSE,
    K_RESULT_OK, TUid, VST3_I_PROGRAM_LIST_DATA_IID, VST3_I_UNIT_DATA_IID, VST3_MEDIA_TYPE_AUDIO,
    parse_tuid_hex, tuid_hex,
};
use crate::{
    DEFAULT_MAX_VST3_STATE_BYTES, HostError, HostResult, Vst3AudioBusInfo, Vst3BusDirection,
    Vst3ConnectionPoint, Vst3ProcessingConfig, Vst3ProgramListData, Vst3UnitData,
    state_stream::Vst3StateStream,
};

use super::select_audio_bus_index;

#[derive(Debug)]
pub(super) struct Vst3ComponentHandle {
    component: NonNull<IComponent>,
}

impl Vst3ComponentHandle {
    pub(super) unsafe fn from_raw(component: *mut c_void) -> HostResult<Self> {
        let component =
            NonNull::new(component.cast::<IComponent>()).ok_or(HostError::ComponentReturnedNull)?;

        // SAFETY: The caller guarantees the pointer is a valid IComponent
        // object for the lifetime transferred to this handle.
        let vtable = unsafe { component.as_ref().vtable };
        if vtable.is_null() {
            return Err(HostError::ComponentVTableMissing);
        }

        Ok(Self { component })
    }

    pub(super) fn initialize(&mut self, context: *mut FUnknown) -> HostResult<()> {
        self.call_result("initialize", |component, vtable| unsafe {
            // SAFETY: The component pointer and vtable were validated by
            // from_raw. `context` is owned by the component holder and remains
            // live until after component termination/drop.
            (vtable.initialize)(component, context)
        })
    }

    pub(super) fn set_active(&mut self, active: bool) -> HostResult<()> {
        let state = if active { 1 } else { 0 };
        self.call_result("setActive", |component, vtable| unsafe {
            // SAFETY: component and vtable were validated by from_raw; VST3
            // TBool accepts 0/1 state values.
            (vtable.set_active)(component, state)
        })
    }

    pub(super) fn activate_audio_buses(
        &mut self,
        config: Vst3ProcessingConfig,
        active: bool,
    ) -> HostResult<()> {
        if config.input_channels > 0 {
            self.activate_selected_audio_bus(
                Vst3BusDirection::Input,
                config.input_channels,
                config.input_bus_index,
                active,
            )?;
        }
        self.activate_selected_audio_bus(
            Vst3BusDirection::Output,
            config.output_channels,
            config.output_bus_index,
            active,
        )
    }

    pub(super) fn controller_class_id(&self) -> HostResult<Option<String>> {
        let mut class_id: TUid = [0; 16];
        let result = unsafe {
            // SAFETY: component and vtable were validated by from_raw; class_id
            // is writable stack storage for the component's controller TUID.
            (self.vtable().get_controller_class_id)(self.component.as_ptr(), &mut class_id)
        };
        match result {
            K_RESULT_OK => Ok((class_id != [0; 16]).then(|| tuid_hex(&class_id))),
            K_RESULT_FALSE | K_NOT_IMPLEMENTED => Ok(None),
            result => Err(HostError::ComponentCallFailed {
                method: "getControllerClassId",
                result,
            }),
        }
    }

    pub(super) fn get_state(&self) -> HostResult<Vec<u8>> {
        let mut stream = Vst3StateStream::bounded_writable(DEFAULT_MAX_VST3_STATE_BYTES);
        let result = unsafe {
            // SAFETY: component/vtable were validated by from_raw. The stream
            // object remains live for the duration of this call.
            (self.vtable().get_state)(self.component.as_ptr(), stream.as_mut_ptr().cast())
        };
        stream.check_write_limit()?;
        if result != K_RESULT_OK {
            return Err(HostError::ComponentCallFailed {
                method: "getState",
                result,
            });
        }
        stream.into_bytes_checked()
    }

    pub(super) fn set_state(&mut self, state: &[u8]) -> HostResult<()> {
        let mut stream = Vst3StateStream::from_bytes(state.to_vec());
        self.call_result("setState", |component, vtable| unsafe {
            // SAFETY: component/vtable were validated by from_raw. The stream
            // object remains live for the duration of this call.
            (vtable.set_state)(component, stream.as_mut_ptr().cast())
        })
    }

    pub(super) fn program_list_data(&self) -> HostResult<Option<Vst3ProgramListData>> {
        let Some(object) = self.query_optional_interface(VST3_I_PROGRAM_LIST_DATA_IID)? else {
            return Ok(None);
        };
        // SAFETY: queryInterface returned a referenced IProgramListData pointer.
        unsafe { Vst3ProgramListData::from_raw(object.cast()) }.map(Some)
    }

    pub(super) fn unit_data(&self) -> HostResult<Option<Vst3UnitData>> {
        let Some(object) = self.query_optional_interface(VST3_I_UNIT_DATA_IID)? else {
            return Ok(None);
        };
        // SAFETY: queryInterface returned a referenced IUnitData pointer.
        unsafe { Vst3UnitData::from_raw(object.cast()) }.map(Some)
    }

    pub(super) fn connection_point(&self) -> HostResult<Option<Vst3ConnectionPoint>> {
        let Some(object) = self.query_optional_interface(Vst3ConnectionPoint::interface_id())?
        else {
            return Ok(None);
        };
        // SAFETY: queryInterface returned a referenced IConnectionPoint pointer.
        unsafe { Vst3ConnectionPoint::from_raw(object.cast()) }.map(Some)
    }

    pub(super) fn audio_buses(
        &mut self,
        direction: Vst3BusDirection,
    ) -> HostResult<Vec<Vst3AudioBusInfo>> {
        let direction_abi = direction.as_abi();
        let count = self.call_count("getBusCount", |component, vtable| unsafe {
            // SAFETY: component and vtable were validated by from_raw.
            (vtable.get_bus_count)(component, VST3_MEDIA_TYPE_AUDIO, direction_abi)
        })?;
        let mut buses = Vec::with_capacity(count as usize);

        for index in 0..count {
            let mut info = BusInfo::default();
            self.call_result("getBusInfo", |component, vtable| unsafe {
                // SAFETY: `info` is stack storage matching the VST3 BusInfo ABI.
                (vtable.get_bus_info)(
                    component,
                    VST3_MEDIA_TYPE_AUDIO,
                    direction_abi,
                    index,
                    (&mut info as *mut BusInfo).cast(),
                )
            })?;
            buses.push(Vst3AudioBusInfo::from_abi(index, direction, info));
        }

        Ok(buses)
    }

    pub(super) fn terminate(&mut self) -> HostResult<()> {
        self.call_result("terminate", |component, vtable| unsafe {
            // SAFETY: component and vtable were validated by from_raw.
            (vtable.terminate)(component)
        })
    }

    fn activate_selected_audio_bus(
        &mut self,
        direction: Vst3BusDirection,
        channels: u16,
        requested_index: Option<i32>,
        active: bool,
    ) -> HostResult<()> {
        let buses = self.audio_buses(direction)?;
        let index = select_audio_bus_index(&buses, channels, requested_index, direction)?;
        self.activate_audio_bus(direction.as_abi(), index, active)
    }

    fn activate_audio_bus(&mut self, direction: i32, index: i32, active: bool) -> HostResult<()> {
        let state = if active { 1 } else { 0 };
        self.call_result("activateBus", |component, vtable| unsafe {
            // SAFETY: component and vtable were validated by from_raw; WVST's
            // MVP process buffers expose one selected audio bus per direction.
            (vtable.activate_bus)(component, VST3_MEDIA_TYPE_AUDIO, direction, index, state)
        })
    }

    fn call_result(
        &mut self,
        method: &'static str,
        call: impl FnOnce(*mut IComponent, &IComponentVTable) -> i32,
    ) -> HostResult<()> {
        let result = call(self.component.as_ptr(), self.vtable());
        if result == K_RESULT_OK {
            Ok(())
        } else {
            Err(HostError::ComponentCallFailed { method, result })
        }
    }

    fn call_count(
        &mut self,
        method: &'static str,
        call: impl FnOnce(*mut IComponent, &IComponentVTable) -> i32,
    ) -> HostResult<i32> {
        let result = call(self.component.as_ptr(), self.vtable());
        if result >= 0 {
            Ok(result)
        } else {
            Err(HostError::ComponentCallFailed { method, result })
        }
    }

    fn query_optional_interface(&self, interface_id: &str) -> HostResult<Option<*mut c_void>> {
        let interface_tuid = parse_tuid_hex(interface_id)
            .ok_or_else(|| HostError::InvalidInterfaceId(interface_id.to_string()))?;
        let mut object: *mut c_void = std::ptr::null_mut();
        let result = unsafe {
            // SAFETY: component/vtable were validated by from_raw. object is
            // writable stack storage for the optional queried interface.
            (self.vtable().query_interface)(
                self.component.as_ptr(),
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

    fn vtable(&self) -> &IComponentVTable {
        // SAFETY: from_raw validated both the object pointer and the vtable
        // pointer. The handle owns the reference until Drop calls release.
        unsafe { &*self.component.as_ref().vtable }
    }
}

impl Drop for Vst3ComponentHandle {
    fn drop(&mut self) {
        let component = self.component.as_ptr();
        let vtable = self.vtable();

        // SAFETY: from_raw transfers one owned IComponent reference to this
        // handle; Drop releases that reference exactly once.
        unsafe {
            (vtable.release)(component);
        }
    }
}
