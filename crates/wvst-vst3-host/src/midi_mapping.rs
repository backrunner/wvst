use std::ptr::NonNull;

use crate::vst3_abi::{
    CtrlNumber, IMidiMapping, IMidiMappingVTable, K_RESULT_FALSE, K_RESULT_OK, ParamId,
};
use crate::{HostError, HostResult};

#[derive(Debug)]
pub struct Vst3MidiMapping {
    mapping: NonNull<IMidiMapping>,
}

impl Vst3MidiMapping {
    /// # Safety
    ///
    /// `mapping` must be an owned, valid VST3 `IMidiMapping` pointer returned
    /// by `queryInterface`. This holder releases that reference on drop.
    pub unsafe fn from_raw(mapping: *mut IMidiMapping) -> HostResult<Self> {
        let mapping = NonNull::new(mapping).ok_or(HostError::InterfaceReturnedNull {
            interface_id: crate::vst3_abi::VST3_I_MIDI_MAPPING_IID.to_string(),
        })?;
        // SAFETY: The caller guarantees `mapping` is a valid interface pointer.
        let vtable = unsafe { mapping.as_ref().vtable };
        if vtable.is_null() {
            return Err(HostError::InterfaceReturnedNull {
                interface_id: crate::vst3_abi::VST3_I_MIDI_MAPPING_IID.to_string(),
            });
        }

        Ok(Self { mapping })
    }

    pub fn assignment(
        &self,
        bus_index: i32,
        channel: u8,
        controller: CtrlNumber,
    ) -> HostResult<Option<ParamId>> {
        let mut id = 0;
        let result = unsafe {
            // SAFETY: mapping/vtable were validated by from_raw; `id` is
            // writable stack storage for the returned ParamID.
            (self.vtable().get_midi_controller_assignment)(
                self.mapping.as_ptr(),
                bus_index,
                i16::from(channel),
                controller,
                &mut id,
            )
        };

        match result {
            K_RESULT_OK => Ok(Some(id)),
            K_RESULT_FALSE => Ok(None),
            result => Err(HostError::EditControllerCallFailed {
                method: "getMidiControllerAssignment",
                result,
            }),
        }
    }

    fn vtable(&self) -> &IMidiMappingVTable {
        // SAFETY: from_raw validates the vtable pointer.
        unsafe { &*self.mapping.as_ref().vtable }
    }
}

impl Drop for Vst3MidiMapping {
    fn drop(&mut self) {
        let mapping = self.mapping.as_ptr();
        let vtable = self.vtable();

        // SAFETY: from_raw transfers one queryInterface reference to this
        // holder; Drop releases that reference exactly once.
        unsafe {
            (vtable.release)(mapping);
        }
    }
}
