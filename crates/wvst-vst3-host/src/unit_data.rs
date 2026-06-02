use std::ptr::NonNull;

use crate::vst3_abi::{
    IProgramListData, IProgramListDataVTable, IUnitData, IUnitDataVTable, K_RESULT_FALSE,
    K_RESULT_OK, ProgramListId, UnitId, VST3_I_PROGRAM_LIST_DATA_IID, VST3_I_UNIT_DATA_IID,
};
use crate::{DEFAULT_MAX_VST3_STATE_BYTES, HostError, HostResult, state_stream::Vst3StateStream};

#[derive(Debug)]
pub struct Vst3ProgramListData {
    data: NonNull<IProgramListData>,
}

#[derive(Debug)]
pub struct Vst3UnitData {
    data: NonNull<IUnitData>,
}

impl Vst3ProgramListData {
    /// # Safety
    ///
    /// `data` must be an owned, valid VST3 `IProgramListData` pointer returned
    /// by `queryInterface`. This holder releases that reference on drop.
    pub unsafe fn from_raw(data: *mut IProgramListData) -> HostResult<Self> {
        let data = NonNull::new(data).ok_or(HostError::InterfaceReturnedNull {
            interface_id: VST3_I_PROGRAM_LIST_DATA_IID.to_string(),
        })?;
        // SAFETY: The caller guarantees `data` is a valid interface pointer.
        let vtable = unsafe { data.as_ref().vtable };
        if vtable.is_null() {
            return Err(HostError::InterfaceReturnedNull {
                interface_id: VST3_I_PROGRAM_LIST_DATA_IID.to_string(),
            });
        }

        Ok(Self { data })
    }

    pub fn program_data_supported(&self, list_id: ProgramListId) -> HostResult<bool> {
        let result = unsafe {
            // SAFETY: data/vtable were validated by from_raw.
            (self.vtable().program_data_supported)(self.data.as_ptr(), list_id)
        };
        bool_result("programDataSupported", result)
    }

    pub fn get_program_data(
        &self,
        list_id: ProgramListId,
        program_index: i32,
    ) -> HostResult<Vec<u8>> {
        let mut stream = Vst3StateStream::bounded_writable(DEFAULT_MAX_VST3_STATE_BYTES);
        let result = unsafe {
            // SAFETY: stream object remains live for the duration of this call.
            (self.vtable().get_program_data)(
                self.data.as_ptr(),
                list_id,
                program_index,
                stream.as_mut_ptr(),
            )
        };
        stream.check_write_limit()?;
        component_result("getProgramData", result)?;
        stream.into_bytes_checked()
    }

    pub fn set_program_data(
        &self,
        list_id: ProgramListId,
        program_index: i32,
        bytes: &[u8],
    ) -> HostResult<()> {
        let mut stream = Vst3StateStream::from_bytes(bytes.to_vec());
        self.call_result("setProgramData", |data, vtable| unsafe {
            // SAFETY: stream object remains live for the duration of this call.
            (vtable.set_program_data)(data, list_id, program_index, stream.as_mut_ptr())
        })
    }

    fn call_result(
        &self,
        method: &'static str,
        call: impl FnOnce(*mut IProgramListData, &IProgramListDataVTable) -> i32,
    ) -> HostResult<()> {
        component_result(method, call(self.data.as_ptr(), self.vtable()))
    }

    fn vtable(&self) -> &IProgramListDataVTable {
        // SAFETY: from_raw validates the vtable pointer.
        unsafe { &*self.data.as_ref().vtable }
    }
}

impl Drop for Vst3ProgramListData {
    fn drop(&mut self) {
        let data = self.data.as_ptr();
        let vtable = self.vtable();

        // SAFETY: from_raw transfers one queryInterface reference to this
        // holder; Drop releases that reference exactly once.
        unsafe {
            (vtable.release)(data);
        }
    }
}

impl Vst3UnitData {
    /// # Safety
    ///
    /// `data` must be an owned, valid VST3 `IUnitData` pointer returned by
    /// `queryInterface`. This holder releases that reference on drop.
    pub unsafe fn from_raw(data: *mut IUnitData) -> HostResult<Self> {
        let data = NonNull::new(data).ok_or(HostError::InterfaceReturnedNull {
            interface_id: VST3_I_UNIT_DATA_IID.to_string(),
        })?;
        // SAFETY: The caller guarantees `data` is a valid interface pointer.
        let vtable = unsafe { data.as_ref().vtable };
        if vtable.is_null() {
            return Err(HostError::InterfaceReturnedNull {
                interface_id: VST3_I_UNIT_DATA_IID.to_string(),
            });
        }

        Ok(Self { data })
    }

    pub fn unit_data_supported(&self, unit_id: UnitId) -> HostResult<bool> {
        let result = unsafe {
            // SAFETY: data/vtable were validated by from_raw.
            (self.vtable().unit_data_supported)(self.data.as_ptr(), unit_id)
        };
        bool_result("unitDataSupported", result)
    }

    pub fn get_unit_data(&self, unit_id: UnitId) -> HostResult<Vec<u8>> {
        let mut stream = Vst3StateStream::bounded_writable(DEFAULT_MAX_VST3_STATE_BYTES);
        let result = unsafe {
            // SAFETY: stream object remains live for the duration of this call.
            (self.vtable().get_unit_data)(self.data.as_ptr(), unit_id, stream.as_mut_ptr())
        };
        stream.check_write_limit()?;
        component_result("getUnitData", result)?;
        stream.into_bytes_checked()
    }

    pub fn set_unit_data(&self, unit_id: UnitId, bytes: &[u8]) -> HostResult<()> {
        let mut stream = Vst3StateStream::from_bytes(bytes.to_vec());
        self.call_result("setUnitData", |data, vtable| unsafe {
            // SAFETY: stream object remains live for the duration of this call.
            (vtable.set_unit_data)(data, unit_id, stream.as_mut_ptr())
        })
    }

    fn call_result(
        &self,
        method: &'static str,
        call: impl FnOnce(*mut IUnitData, &IUnitDataVTable) -> i32,
    ) -> HostResult<()> {
        component_result(method, call(self.data.as_ptr(), self.vtable()))
    }

    fn vtable(&self) -> &IUnitDataVTable {
        // SAFETY: from_raw validates the vtable pointer.
        unsafe { &*self.data.as_ref().vtable }
    }
}

impl Drop for Vst3UnitData {
    fn drop(&mut self) {
        let data = self.data.as_ptr();
        let vtable = self.vtable();

        // SAFETY: from_raw transfers one queryInterface reference to this
        // holder; Drop releases that reference exactly once.
        unsafe {
            (vtable.release)(data);
        }
    }
}

fn bool_result(method: &'static str, result: i32) -> HostResult<bool> {
    match result {
        K_RESULT_OK => Ok(true),
        K_RESULT_FALSE => Ok(false),
        result => Err(HostError::ComponentCallFailed { method, result }),
    }
}

fn component_result(method: &'static str, result: i32) -> HostResult<()> {
    if result == K_RESULT_OK {
        Ok(())
    } else {
        Err(HostError::ComponentCallFailed { method, result })
    }
}

#[cfg(test)]
#[path = "unit_data_tests.rs"]
mod tests;
