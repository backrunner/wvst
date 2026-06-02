use std::ffi::c_void;
use std::ptr::NonNull;

use crate::vst3_abi::{
    IConnectionPoint, IConnectionPointVTable, K_RESULT_OK, VST3_I_CONNECTION_POINT_IID,
};
use crate::{HostError, HostResult};

#[derive(Debug)]
pub struct Vst3ConnectionPoint {
    point: NonNull<IConnectionPoint>,
}

impl Vst3ConnectionPoint {
    /// # Safety
    ///
    /// `point` must be an owned, valid VST3 `IConnectionPoint` pointer.
    /// This holder releases that reference on drop.
    pub unsafe fn from_raw(point: *mut c_void) -> HostResult<Self> {
        let point = NonNull::new(point.cast::<IConnectionPoint>())
            .ok_or(HostError::ConnectionPointReturnedNull)?;

        // SAFETY: The caller guarantees the pointer is a valid IConnectionPoint
        // object for the lifetime transferred to this holder.
        let vtable = unsafe { point.as_ref().vtable };
        if vtable.is_null() {
            return Err(HostError::ConnectionPointVTableMissing);
        }

        Ok(Self { point })
    }

    pub fn interface_id() -> &'static str {
        VST3_I_CONNECTION_POINT_IID
    }

    pub fn connect(&self, other: &Self) -> HostResult<()> {
        self.call_result("connect", |point, vtable| unsafe {
            // SAFETY: both connection point holders validated their pointers
            // and own live references for the duration of this call.
            (vtable.connect)(point, other.as_mut_ptr())
        })
    }

    pub fn disconnect(&self, other: &Self) -> HostResult<()> {
        self.call_result("disconnect", |point, vtable| unsafe {
            // SAFETY: both connection point holders validated their pointers
            // and own live references for the duration of this call.
            (vtable.disconnect)(point, other.as_mut_ptr())
        })
    }

    pub fn as_mut_ptr(&self) -> *mut IConnectionPoint {
        self.point.as_ptr()
    }

    fn call_result(
        &self,
        method: &'static str,
        call: impl FnOnce(*mut IConnectionPoint, &IConnectionPointVTable) -> i32,
    ) -> HostResult<()> {
        let result = call(self.point.as_ptr(), self.vtable());
        if result == K_RESULT_OK {
            Ok(())
        } else {
            Err(HostError::ConnectionPointCallFailed { method, result })
        }
    }

    fn vtable(&self) -> &IConnectionPointVTable {
        // SAFETY: from_raw validated both the object pointer and vtable pointer.
        unsafe { &*self.point.as_ref().vtable }
    }
}

impl Drop for Vst3ConnectionPoint {
    fn drop(&mut self) {
        let point = self.point.as_ptr();
        let vtable = self.vtable();

        // SAFETY: from_raw transfers one owned IConnectionPoint reference to
        // this holder; Drop releases that reference exactly once.
        unsafe {
            (vtable.release)(point);
        }
    }
}

#[cfg(test)]
#[path = "connection_point_tests.rs"]
mod tests;
