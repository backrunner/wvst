use std::ffi::c_void;

use super::{Vst3FactoryClass, Vst3FactoryInfo};
use crate::vst3_abi::{
    FUnknown, IPluginFactory, K_RESULT_OK, PClassInfo, PFactoryInfo, fixed_string, parse_tuid_hex,
    tuid_hex,
};
use crate::{HostError, HostResult};

pub(super) struct PluginFactory {
    factory: *mut IPluginFactory,
}

impl PluginFactory {
    /// # Safety
    ///
    /// `factory` must be a non-null owned `IPluginFactory` pointer returned by
    /// the loaded VST3 module's `GetPluginFactory` entry point. This wrapper
    /// releases the factory exactly once when dropped.
    pub(super) unsafe fn from_raw(factory: *mut IPluginFactory) -> HostResult<Self> {
        if factory.is_null() {
            return Err(HostError::FactoryReturnedNull);
        }

        Ok(Self { factory })
    }

    pub(super) fn info(&self) -> HostResult<Vst3FactoryInfo> {
        let vtable = self.vtable()?;
        let mut info = PFactoryInfo::default();

        // SAFETY: `self.factory` is a non-null IPluginFactory pointer returned
        // by GetPluginFactory. `info` points to writable stack storage.
        let result = unsafe { ((*vtable).get_factory_info)(self.factory, &mut info) };
        if result != K_RESULT_OK {
            return Err(HostError::FactoryCallFailed {
                method: "getFactoryInfo",
                result,
            });
        }

        let classes = self.classes()?;

        Ok(Vst3FactoryInfo {
            vendor: fixed_string(&info.vendor),
            url: fixed_string(&info.url),
            email: fixed_string(&info.email),
            flags: info.flags,
            classes,
        })
    }

    pub(super) fn create_instance(
        &self,
        class_id: &str,
        interface_id: &str,
    ) -> HostResult<Vst3UnknownInstance> {
        let vtable = self.vtable()?;
        let (class_tuid, interface_tuid) =
            super::factory_create_instance_tuids(class_id, interface_id)?;
        let mut object: *mut c_void = std::ptr::null_mut();

        // SAFETY: `self.factory` is a valid IPluginFactory pointer. The class
        // and interface ids point to stable 16-byte TUID buffers for the
        // duration of the call, and `object` points to writable stack storage
        // for the returned FUnknown pointer.
        let result = unsafe {
            ((*vtable).create_instance)(
                self.factory,
                class_tuid.as_ptr().cast(),
                interface_tuid.as_ptr().cast(),
                &mut object,
            )
        };
        if result != K_RESULT_OK {
            return Err(HostError::InstanceCreationFailed {
                class_id: class_id.to_string(),
                interface_id: interface_id.to_string(),
                result,
            });
        }
        if object.is_null() {
            return Err(HostError::InstanceReturnedNull {
                class_id: class_id.to_string(),
                interface_id: interface_id.to_string(),
            });
        }

        Ok(Vst3UnknownInstance {
            object: object.cast(),
        })
    }

    fn classes(&self) -> HostResult<Vec<Vst3FactoryClass>> {
        let vtable = self.vtable()?;

        // SAFETY: `self.factory` is a valid IPluginFactory pointer.
        let count = unsafe { ((*vtable).count_classes)(self.factory) };
        if count < 0 {
            return Err(HostError::FactoryCallFailed {
                method: "countClasses",
                result: count,
            });
        }

        let mut classes = Vec::with_capacity(count as usize);
        for index in 0..count {
            let mut class_info = PClassInfo::default();

            // SAFETY: `class_info` points to writable stack storage and index
            // is bounded by the factory-reported class count.
            let result =
                unsafe { ((*vtable).get_class_info)(self.factory, index, &mut class_info) };
            if result != K_RESULT_OK {
                return Err(HostError::FactoryCallFailed {
                    method: "getClassInfo",
                    result,
                });
            }

            classes.push(Vst3FactoryClass {
                class_id: tuid_hex(&class_info.cid),
                cardinality: class_info.cardinality,
                category: fixed_string(&class_info.category),
                name: fixed_string(&class_info.name),
            });
        }

        Ok(classes)
    }

    fn vtable(&self) -> HostResult<*const crate::vst3_abi::IPluginFactoryVTable> {
        // SAFETY: only reads the vtable pointer from a non-null factory pointer.
        let vtable = unsafe { (*self.factory).vtable };
        if vtable.is_null() {
            Err(HostError::FactoryReturnedNull)
        } else {
            Ok(vtable)
        }
    }
}

impl Drop for PluginFactory {
    fn drop(&mut self) {
        if let Ok(vtable) = self.vtable() {
            // SAFETY: release balances the factory reference returned by
            // GetPluginFactory.
            let _ = unsafe { ((*vtable).release)(self.factory) };
        }
    }
}

pub(super) struct Vst3UnknownInstance {
    object: *mut FUnknown,
}

impl Vst3UnknownInstance {
    pub(super) fn into_raw(self) -> *mut FUnknown {
        let object = self.object;
        std::mem::forget(self);
        object
    }

    pub(super) fn supports_interface(&self, interface_id: &str) -> HostResult<bool> {
        let object = match self.query_interface_owned(interface_id) {
            Ok(object) => object,
            Err(
                HostError::InterfaceQueryFailed { .. } | HostError::InterfaceReturnedNull { .. },
            ) => {
                return Ok(false);
            }
            Err(error) => return Err(error),
        };

        release_unknown(object.cast());
        Ok(true)
    }

    pub(super) fn query_interface_owned(&self, interface_id: &str) -> HostResult<*mut c_void> {
        let interface_tuid = parse_tuid_hex(interface_id)
            .ok_or_else(|| HostError::InvalidInterfaceId(interface_id.to_string()))?;
        let mut object: *mut c_void = std::ptr::null_mut();

        // SAFETY: `self.object` is a live FUnknown-derived interface pointer.
        // `object` points to writable stack storage and is released below if
        // queryInterface returns a referenced object.
        let vtable = unsafe { (*self.object).vtable };
        if vtable.is_null() {
            return Err(HostError::InterfaceReturnedNull {
                interface_id: interface_id.to_string(),
            });
        }

        // SAFETY: queryInterface expects a 16-byte TUID pointer that remains
        // alive for the duration of this call.
        let result = unsafe {
            ((*vtable).query_interface)(self.object, interface_tuid.as_ptr().cast(), &mut object)
        };

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

        Ok(object)
    }
}

impl Drop for Vst3UnknownInstance {
    fn drop(&mut self) {
        release_unknown(self.object);
    }
}

fn release_unknown(object: *mut FUnknown) {
    if object.is_null() {
        return;
    }

    // SAFETY: `object` was returned by IPluginFactory::createInstance or
    // queryInterface for an interface derived from FUnknown. VST3 interfaces
    // expose release as the third FUnknown vtable entry. Null vtables are
    // ignored because there is nothing safe to release.
    let vtable = unsafe { (*object).vtable };
    if !vtable.is_null() {
        // SAFETY: release balances the reference returned by createInstance
        // or queryInterface.
        let _ = unsafe { ((*vtable).release)(object) };
    }
}
