use std::ffi::{CString, c_void};
use std::path::Path;

use core_foundation_sys::base::{Boolean, CFRelease, kCFAllocatorDefault};
use core_foundation_sys::bundle::{
    CFBundleCreate, CFBundleGetFunctionPointerForName, CFBundleLoadExecutable, CFBundleRef,
    CFBundleUnloadExecutable,
};
use core_foundation_sys::string::{CFStringCreateWithCString, CFStringRef, kCFStringEncodingUTF8};
use core_foundation_sys::url::{CFURLCreateWithFileSystemPath, kCFURLPOSIXPathStyle};

use super::{Vst3FactoryInfo, plugin_factory::PluginFactory};
use crate::vst3_abi::{IPluginFactory, VST3_I_AUDIO_PROCESSOR_IID, VST3_I_EDIT_CONTROLLER_IID};
use crate::{
    HostError, HostResult, Vst3ComponentInstance, Vst3EditController, Vst3ProcessingConfig,
};

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
    let controller = match instance.controller_class_id()? {
        Some(controller_class_id) => {
            let controller =
                factory.create_instance(&controller_class_id, VST3_I_EDIT_CONTROLLER_IID)?;
            Some(unsafe { Vst3EditController::from_raw(controller.into_raw().cast()) }?)
        }
        None => None,
    };

    Ok(super::Vst3LoadedComponent::new(
        instance,
        controller,
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

        // SAFETY: `factory` was returned by the VST3 module's documented
        // GetPluginFactory accessor after a successful bundleEntry call.
        unsafe { PluginFactory::from_raw(factory) }
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
