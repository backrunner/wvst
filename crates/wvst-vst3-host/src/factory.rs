use serde::{Deserialize, Serialize};

use crate::{HostResult, Vst3ComponentInstance, Vst3ProcessingConfig};

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

pub struct Vst3LoadedComponent {
    instance: Vst3ComponentInstance,
    _module: platform::LoadedPluginModule,
}

impl Vst3LoadedComponent {
    fn new(instance: Vst3ComponentInstance, module: platform::LoadedPluginModule) -> Self {
        Self {
            instance,
            _module: module,
        }
    }

    pub fn instance(&self) -> &Vst3ComponentInstance {
        &self.instance
    }

    pub fn instance_mut(&mut self) -> &mut Vst3ComponentInstance {
        &mut self.instance
    }
}

// SAFETY: `Vst3LoadedComponent` is an owning runtime holder. Its raw plugin
// pointers are released exactly once by their wrappers, and process-buffer ABI
// pointers target heap allocations owned by the same holder. Moving the holder
// to another thread does not invalidate those allocations. The type is not
// `Sync`; callers still need exclusive mutable access for lifecycle and process
// calls, and WVST workers additionally serialize access with their instance
// mutex.
unsafe impl Send for Vst3LoadedComponent {}

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
mod platform {
    use std::ffi::{CString, c_void};
    use std::path::Path;

    use core_foundation_sys::base::{Boolean, CFRelease, kCFAllocatorDefault};
    use core_foundation_sys::bundle::{
        CFBundleCreate, CFBundleGetFunctionPointerForName, CFBundleLoadExecutable, CFBundleRef,
        CFBundleUnloadExecutable,
    };
    use core_foundation_sys::string::{
        CFStringCreateWithCString, CFStringRef, kCFStringEncodingUTF8,
    };
    use core_foundation_sys::url::{CFURLCreateWithFileSystemPath, kCFURLPOSIXPathStyle};

    use super::{Vst3FactoryClass, Vst3FactoryInfo};
    use crate::vst3_abi::{
        FUnknown, IPluginFactory, K_RESULT_OK, PClassInfo, PFactoryInfo,
        VST3_I_AUDIO_PROCESSOR_IID, fixed_string, parse_tuid_hex, tuid_hex,
    };
    use crate::{HostError, HostResult, Vst3ComponentInstance, Vst3ProcessingConfig};

    type BundleEntry = unsafe extern "C" fn(bundle: CFBundleRef) -> Boolean;
    type BundleExit = unsafe extern "C" fn() -> Boolean;
    type GetPluginFactory = unsafe extern "C" fn() -> *mut IPluginFactory;

    pub fn load_vst3_factory_info(bundle_path: &Path) -> HostResult<Vst3FactoryInfo> {
        let mut bundle = MacBundle::open(bundle_path)?;
        bundle.call_entry()?;

        let factory = bundle.plugin_factory()?;
        let info = factory.info()?;

        Ok(info)
    }

    pub fn create_vst3_component_probe(
        bundle_path: &Path,
        class_id: &str,
        interface_id: &str,
    ) -> HostResult<super::Vst3ComponentProbe> {
        let mut bundle = MacBundle::open(bundle_path)?;
        bundle.call_entry()?;

        let factory = bundle.plugin_factory()?;
        let instance = factory.create_instance(class_id, interface_id)?;
        let audio_processor = instance.supports_interface(VST3_I_AUDIO_PROCESSOR_IID)?;
        drop(instance);

        Ok(super::Vst3ComponentProbe {
            bundle_path: bundle_path.to_string_lossy().into_owned(),
            class_id: class_id.to_string(),
            interface_id: interface_id.to_string(),
            audio_processor,
            created: true,
        })
    }

    pub fn create_vst3_component_instance(
        bundle_path: &Path,
        class_id: &str,
        processing_config: Vst3ProcessingConfig,
    ) -> HostResult<super::Vst3LoadedComponent> {
        let mut bundle = MacBundle::open(bundle_path)?;
        bundle.call_entry()?;

        let factory = bundle.plugin_factory()?;
        let component = factory.create_instance(class_id, crate::vst3_abi::VST3_I_COMPONENT_IID)?;
        let processor = component.query_interface_owned(VST3_I_AUDIO_PROCESSOR_IID)?;
        let instance = unsafe {
            Vst3ComponentInstance::from_raw_parts(
                component.into_raw().cast(),
                processor,
                processing_config,
            )
        }?;

        Ok(super::Vst3LoadedComponent::new(
            instance,
            LoadedPluginModule { _bundle: bundle },
        ))
    }

    pub(super) struct LoadedPluginModule {
        _bundle: MacBundle,
    }

    struct MacBundle {
        bundle: CFBundleRef,
        entry_called: bool,
    }

    impl MacBundle {
        fn open(path: &Path) -> HostResult<Self> {
            let path_string = path.to_string_lossy();
            let path_cstring = CString::new(path_string.as_bytes())
                .map_err(|error| HostError::ModuleLoadFailed(error.to_string()))?;

            // SAFETY: CoreFoundation objects are created from validated C strings
            // and released on every non-null path before returning or in Drop.
            let cf_path = unsafe {
                CFStringCreateWithCString(
                    kCFAllocatorDefault,
                    path_cstring.as_ptr(),
                    kCFStringEncodingUTF8,
                )
            };
            if cf_path.is_null() {
                return Err(HostError::ModuleLoadFailed(
                    "failed to create CFString for VST3 path".to_string(),
                ));
            }

            // SAFETY: `cf_path` is a live CFString. The path is a directory bundle.
            let url = unsafe {
                CFURLCreateWithFileSystemPath(
                    kCFAllocatorDefault,
                    cf_path,
                    kCFURLPOSIXPathStyle,
                    true as Boolean,
                )
            };
            // SAFETY: `cf_path` is no longer needed after CFURL creation.
            unsafe { CFRelease(cf_path.cast()) };

            if url.is_null() {
                return Err(HostError::ModuleLoadFailed(
                    "failed to create CFURL for VST3 bundle".to_string(),
                ));
            }

            // SAFETY: `url` points to a bundle directory URL.
            let bundle = unsafe { CFBundleCreate(kCFAllocatorDefault, url) };
            // SAFETY: `url` is no longer needed after CFBundle creation.
            unsafe { CFRelease(url.cast()) };

            if bundle.is_null() {
                return Err(HostError::ModuleLoadFailed(
                    "failed to create CFBundle for VST3 bundle".to_string(),
                ));
            }

            // SAFETY: `bundle` is a live CFBundleRef.
            let loaded = unsafe { CFBundleLoadExecutable(bundle) };
            if loaded == 0 {
                // SAFETY: `bundle` was created above and must be released on failure.
                unsafe { CFRelease(bundle.cast()) };
                return Err(HostError::ModuleLoadFailed(
                    "CFBundleLoadExecutable failed".to_string(),
                ));
            }

            Ok(Self {
                bundle,
                entry_called: false,
            })
        }

        fn call_entry(&mut self) -> HostResult<()> {
            let entry = self.symbol::<BundleEntry>("bundleEntry")?;

            // SAFETY: `bundleEntry` is the VST3 macOS module entry point and is
            // called once with the live CFBundleRef that loaded the executable.
            let ok = unsafe { entry(self.bundle) };
            if ok == 0 {
                return Err(HostError::FactoryCallFailed {
                    method: "bundleEntry",
                    result: i32::from(ok),
                });
            }

            self.entry_called = true;
            Ok(())
        }

        fn plugin_factory(&self) -> HostResult<PluginFactory> {
            let get_plugin_factory = self.symbol::<GetPluginFactory>("GetPluginFactory")?;

            // SAFETY: The VST3 module has been entered successfully, and the
            // symbol is the documented factory accessor. Null is handled below.
            let factory = unsafe { get_plugin_factory() };
            if factory.is_null() {
                return Err(HostError::FactoryReturnedNull);
            }

            Ok(PluginFactory { factory })
        }

        fn symbol<T>(&self, name: &'static str) -> HostResult<T>
        where
            T: Copy,
        {
            let cf_name = cf_string(name)?;

            // SAFETY: `self.bundle` is loaded and `cf_name` is a live CFString.
            let pointer = unsafe { CFBundleGetFunctionPointerForName(self.bundle, cf_name) };
            // SAFETY: `cf_name` is no longer needed after symbol lookup.
            unsafe { CFRelease(cf_name.cast()) };

            if pointer.is_null() {
                return Err(HostError::MissingSymbol(name));
            }

            // SAFETY: The requested symbol name determines the expected function
            // pointer type. Call sites only request documented VST3 entry points.
            Ok(unsafe { std::mem::transmute_copy::<*const c_void, T>(&pointer) })
        }
    }

    impl Drop for MacBundle {
        fn drop(&mut self) {
            if self.entry_called {
                if let Ok(exit) = self.symbol::<BundleExit>("bundleExit") {
                    // SAFETY: `bundleExit` is paired with a successful `bundleEntry`.
                    let _ = unsafe { exit() };
                }
            }

            // SAFETY: `bundle` was loaded by CFBundleLoadExecutable and is live.
            unsafe {
                CFBundleUnloadExecutable(self.bundle);
                CFRelease(self.bundle.cast());
            }
        }
    }

    struct PluginFactory {
        factory: *mut IPluginFactory,
    }

    impl PluginFactory {
        fn info(&self) -> HostResult<Vst3FactoryInfo> {
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

        fn create_instance(
            &self,
            class_id: &str,
            interface_id: &str,
        ) -> HostResult<Vst3UnknownInstance> {
            let vtable = self.vtable()?;
            let class_id_string = CString::new(class_id)
                .map_err(|error| HostError::ModuleLoadFailed(error.to_string()))?;
            let interface_id_string = CString::new(interface_id)
                .map_err(|error| HostError::ModuleLoadFailed(error.to_string()))?;
            let mut object: *mut c_void = std::ptr::null_mut();

            // SAFETY: `self.factory` is a valid IPluginFactory pointer. The class
            // and interface ids are NUL-terminated FUID strings, and `object`
            // points to writable stack storage for the returned FUnknown pointer.
            let result = unsafe {
                ((*vtable).create_instance)(
                    self.factory,
                    class_id_string.as_ptr(),
                    interface_id_string.as_ptr(),
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

    struct Vst3UnknownInstance {
        object: *mut FUnknown,
    }

    impl Drop for Vst3UnknownInstance {
        fn drop(&mut self) {
            release_unknown(self.object);
        }
    }

    impl Vst3UnknownInstance {
        fn into_raw(self) -> *mut FUnknown {
            let object = self.object;
            std::mem::forget(self);
            object
        }

        fn supports_interface(&self, interface_id: &str) -> HostResult<bool> {
            let object = match self.query_interface_owned(interface_id) {
                Ok(object) => object,
                Err(
                    HostError::InterfaceQueryFailed { .. }
                    | HostError::InterfaceReturnedNull { .. },
                ) => {
                    return Ok(false);
                }
                Err(error) => return Err(error),
            };

            release_unknown(object.cast());
            Ok(true)
        }

        fn query_interface_owned(&self, interface_id: &str) -> HostResult<*mut c_void> {
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
            // SAFETY: queryInterface expects a 16-byte TUID pointer that
            // remains alive for the duration of this call.
            let result = unsafe {
                ((*vtable).query_interface)(
                    self.object,
                    interface_tuid.as_ptr().cast(),
                    &mut object,
                )
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

    impl Drop for PluginFactory {
        fn drop(&mut self) {
            if let Ok(vtable) = self.vtable() {
                // SAFETY: release balances the factory reference returned by
                // GetPluginFactory.
                let _ = unsafe { ((*vtable).release)(self.factory) };
            }
        }
    }

    fn cf_string(value: &'static str) -> HostResult<CFStringRef> {
        let cstring =
            CString::new(value).map_err(|error| HostError::ModuleLoadFailed(error.to_string()))?;

        // SAFETY: `cstring` is NUL-terminated and valid for this call.
        let string = unsafe {
            CFStringCreateWithCString(kCFAllocatorDefault, cstring.as_ptr(), kCFStringEncodingUTF8)
        };

        if string.is_null() {
            Err(HostError::ModuleLoadFailed(format!(
                "failed to create CFString for symbol {value}"
            )))
        } else {
            Ok(string)
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use std::path::Path;

    use super::{Vst3ComponentProbe, Vst3FactoryInfo};
    use crate::{HostError, HostResult, Vst3ProcessingConfig};

    pub(super) struct LoadedPluginModule;

    pub fn load_vst3_factory_info(_bundle_path: &Path) -> HostResult<Vst3FactoryInfo> {
        Err(HostError::UnsupportedPlatform(
            "VST3 factory loading is currently implemented for macOS only",
        ))
    }

    pub fn create_vst3_component_probe(
        _bundle_path: &Path,
        _class_id: &str,
        _interface_id: &str,
    ) -> HostResult<Vst3ComponentProbe> {
        Err(HostError::UnsupportedPlatform(
            "VST3 createInstance is currently implemented for macOS only",
        ))
    }

    pub fn create_vst3_component_instance(
        _bundle_path: &Path,
        _class_id: &str,
        _processing_config: Vst3ProcessingConfig,
    ) -> HostResult<super::Vst3LoadedComponent> {
        Err(HostError::UnsupportedPlatform(
            "VST3 createInstance is currently implemented for macOS only",
        ))
    }
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
