use std::ptr::NonNull;

use serde::{Deserialize, Serialize};

use crate::edit_controller::string128_to_string;
use crate::vst3_abi::{
    IUnitInfo, IUnitInfoVTable, K_RESULT_FALSE, K_RESULT_OK, ProgramListId,
    ProgramListInfo as RawProgramListInfo, UnitId, UnitInfo as RawUnitInfo, VST3_I_UNIT_INFO_IID,
    VST3_MEDIA_TYPE_AUDIO, VST3_NO_PROGRAM_LIST_ID,
};
use crate::{HostError, HostResult, Vst3BusDirection, state_stream::Vst3StateStream};

#[derive(Debug)]
pub struct Vst3UnitInfo {
    unit_info: NonNull<IUnitInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3UnitMetadata {
    pub units: Vec<Vst3UnitInfoEntry>,
    pub program_lists: Vec<Vst3ProgramList>,
    pub selected_unit_id: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3UnitInfoEntry {
    pub id: i32,
    pub parent_unit_id: i32,
    pub name: Option<String>,
    pub program_list_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3ProgramList {
    pub id: i32,
    pub name: Option<String>,
    pub program_count: i32,
    pub programs: Vec<Vst3ProgramInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3ProgramInfo {
    pub index: i32,
    pub name: Option<String>,
}

impl Vst3UnitInfo {
    /// # Safety
    ///
    /// `unit_info` must be an owned, valid VST3 `IUnitInfo` pointer returned
    /// by `queryInterface`. This holder releases that reference on drop.
    pub unsafe fn from_raw(unit_info: *mut IUnitInfo) -> HostResult<Self> {
        let unit_info = NonNull::new(unit_info).ok_or(HostError::InterfaceReturnedNull {
            interface_id: VST3_I_UNIT_INFO_IID.to_string(),
        })?;
        // SAFETY: The caller guarantees `unit_info` is a valid interface pointer.
        let vtable = unsafe { unit_info.as_ref().vtable };
        if vtable.is_null() {
            return Err(HostError::InterfaceReturnedNull {
                interface_id: VST3_I_UNIT_INFO_IID.to_string(),
            });
        }

        Ok(Self { unit_info })
    }

    pub fn metadata(&self) -> HostResult<Vst3UnitMetadata> {
        Ok(Vst3UnitMetadata {
            units: self.units()?,
            program_lists: self.program_lists()?,
            selected_unit_id: self.selected_unit(),
        })
    }

    pub fn unit_count(&self) -> HostResult<i32> {
        non_negative_count("getUnitCount", unsafe {
            // SAFETY: unit_info/vtable were validated by from_raw.
            (self.vtable().get_unit_count)(self.unit_info.as_ptr())
        })
    }

    pub fn unit_info(&self, index: i32) -> HostResult<Vst3UnitInfoEntry> {
        let mut info = RawUnitInfo::default();
        self.call_result("getUnitInfo", |unit_info, vtable| unsafe {
            // SAFETY: info is writable stack storage matching VST3 ABI.
            (vtable.get_unit_info)(unit_info, index, &mut info)
        })?;
        Ok(Vst3UnitInfoEntry::from_abi(info))
    }

    pub fn units(&self) -> HostResult<Vec<Vst3UnitInfoEntry>> {
        let count = self.unit_count()?;
        let mut units = Vec::with_capacity(count as usize);
        for index in 0..count {
            units.push(self.unit_info(index)?);
        }
        Ok(units)
    }

    pub fn program_list_count(&self) -> HostResult<i32> {
        non_negative_count("getProgramListCount", unsafe {
            // SAFETY: unit_info/vtable were validated by from_raw.
            (self.vtable().get_program_list_count)(self.unit_info.as_ptr())
        })
    }

    pub fn program_list_info(&self, index: i32) -> HostResult<Vst3ProgramList> {
        let mut info = RawProgramListInfo::default();
        self.call_result("getProgramListInfo", |unit_info, vtable| unsafe {
            // SAFETY: info is writable stack storage matching VST3 ABI.
            (vtable.get_program_list_info)(unit_info, index, &mut info)
        })?;
        if info.program_count < 0 {
            return Err(HostError::EditControllerCallFailed {
                method: "getProgramListInfo.programCount",
                result: info.program_count,
            });
        }

        let mut programs = Vec::with_capacity(info.program_count as usize);
        for program_index in 0..info.program_count {
            programs.push(Vst3ProgramInfo {
                index: program_index,
                name: self.program_name(info.id, program_index)?,
            });
        }

        Ok(Vst3ProgramList {
            id: info.id,
            name: string128_to_string(&info.name),
            program_count: info.program_count,
            programs,
        })
    }

    pub fn program_lists(&self) -> HostResult<Vec<Vst3ProgramList>> {
        let count = self.program_list_count()?;
        let mut lists = Vec::with_capacity(count as usize);
        for index in 0..count {
            lists.push(self.program_list_info(index)?);
        }
        Ok(lists)
    }

    pub fn program_name(
        &self,
        list_id: ProgramListId,
        program_index: i32,
    ) -> HostResult<Option<String>> {
        let mut name = [0; 128];
        self.call_result("getProgramName", |unit_info, vtable| unsafe {
            // SAFETY: name is writable String128 storage matching VST3 ABI.
            (vtable.get_program_name)(unit_info, list_id, program_index, &mut name)
        })?;
        Ok(string128_to_string(&name))
    }

    pub fn selected_unit(&self) -> UnitId {
        unsafe {
            // SAFETY: unit_info/vtable were validated by from_raw.
            (self.vtable().get_selected_unit)(self.unit_info.as_ptr())
        }
    }

    pub fn select_unit(&self, unit_id: UnitId) -> HostResult<()> {
        self.call_result("selectUnit", |unit_info, vtable| unsafe {
            // SAFETY: unit_info/vtable were validated by from_raw.
            (vtable.select_unit)(unit_info, unit_id)
        })
    }

    pub fn unit_by_audio_bus(
        &self,
        direction: Vst3BusDirection,
        bus_index: i32,
        channel: i32,
    ) -> HostResult<Option<UnitId>> {
        let mut unit_id = 0;
        let result = unsafe {
            // SAFETY: unit_id points to writable stack storage, and
            // unit_info/vtable were validated by from_raw.
            (self.vtable().get_unit_by_bus)(
                self.unit_info.as_ptr(),
                VST3_MEDIA_TYPE_AUDIO,
                direction.as_abi(),
                bus_index,
                channel,
                &mut unit_id,
            )
        };
        match result {
            K_RESULT_OK => Ok(Some(unit_id)),
            K_RESULT_FALSE => Ok(None),
            result => Err(HostError::EditControllerCallFailed {
                method: "getUnitByBus",
                result,
            }),
        }
    }

    pub fn set_unit_program_data(
        &self,
        list_or_unit_id: i32,
        program_index: i32,
        data: &[u8],
    ) -> HostResult<()> {
        let mut stream = Vst3StateStream::from_bytes(data.to_vec());
        self.call_result("setUnitProgramData", |unit_info, vtable| unsafe {
            // SAFETY: stream object remains live for the duration of this call.
            (vtable.set_unit_program_data)(
                unit_info,
                list_or_unit_id,
                program_index,
                stream.as_mut_ptr(),
            )
        })
    }

    fn call_result(
        &self,
        method: &'static str,
        call: impl FnOnce(*mut IUnitInfo, &IUnitInfoVTable) -> i32,
    ) -> HostResult<()> {
        let result = call(self.unit_info.as_ptr(), self.vtable());
        if result == K_RESULT_OK {
            Ok(())
        } else {
            Err(HostError::EditControllerCallFailed { method, result })
        }
    }

    fn vtable(&self) -> &IUnitInfoVTable {
        // SAFETY: from_raw validates the vtable pointer.
        unsafe { &*self.unit_info.as_ref().vtable }
    }
}

impl Drop for Vst3UnitInfo {
    fn drop(&mut self) {
        let unit_info = self.unit_info.as_ptr();
        let vtable = self.vtable();

        // SAFETY: from_raw transfers one queryInterface reference to this
        // holder; Drop releases that reference exactly once.
        unsafe {
            (vtable.release)(unit_info);
        }
    }
}

impl Vst3UnitInfoEntry {
    fn from_abi(info: RawUnitInfo) -> Self {
        Self {
            id: info.id,
            parent_unit_id: info.parent_unit_id,
            name: string128_to_string(&info.name),
            program_list_id: (info.program_list_id != VST3_NO_PROGRAM_LIST_ID)
                .then_some(info.program_list_id),
        }
    }
}

fn non_negative_count(method: &'static str, count: i32) -> HostResult<i32> {
    if count >= 0 {
        Ok(count)
    } else {
        Err(HostError::EditControllerCallFailed {
            method,
            result: count,
        })
    }
}

#[cfg(test)]
#[path = "unit_info_tests.rs"]
mod tests;
