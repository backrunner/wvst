use std::path::Path;

#[cfg(all(unix, not(target_os = "macos")))]
use std::ffi::c_void;

#[cfg(all(unix, not(target_os = "macos")))]
use libloading::os::unix::Library as PlatformLibrary;
#[cfg(target_os = "windows")]
use libloading::os::windows::Library as PlatformLibrary;

use super::{Vst3FactoryInfo, plugin_factory::PluginFactory};
use crate::vst3_abi::{IPluginFactory, VST3_I_AUDIO_PROCESSOR_IID, VST3_I_EDIT_CONTROLLER_IID};
use crate::{
    HostError, HostResult, Vst3ComponentInstance, Vst3EditController, Vst3ProcessingConfig,
};

#[cfg(target_os = "windows")]
type InitDll = unsafe extern "system" fn() -> bool;
#[cfg(target_os = "windows")]
type ExitDll = unsafe extern "system" fn() -> bool;
#[cfg(target_os = "windows")]
type GetPluginFactory = unsafe extern "system" fn() -> *mut IPluginFactory;

#[cfg(all(unix, not(target_os = "macos")))]
type ModuleEntry = unsafe extern "C" fn(*mut c_void) -> bool;
#[cfg(all(unix, not(target_os = "macos")))]
type ModuleExit = unsafe extern "C" fn() -> bool;
#[cfg(all(unix, not(target_os = "macos")))]
type GetPluginFactory = unsafe extern "C" fn() -> *mut IPluginFactory;

pub fn load_vst3_factory_info(bundle_path: &Path) -> HostResult<Vst3FactoryInfo> {
    let mut module = NativePluginModule::open(bundle_path)?;
    module.call_entry()?;

    let factory = module.plugin_factory()?;
    let info = factory.info()?;

    Ok(info)
}

pub fn create_vst3_component_probe(
    bundle_path: &Path,
    class_id: &str,
    interface_id: &str,
) -> HostResult<super::Vst3ComponentProbe> {
    let mut module = NativePluginModule::open(bundle_path)?;
    module.call_entry()?;

    let factory = module.plugin_factory()?;
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
    let mut module = NativePluginModule::open(bundle_path)?;
    module.call_entry()?;

    let factory = module.plugin_factory()?;
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
        LoadedPluginModule { _module: module },
    ))
}

pub(super) struct LoadedPluginModule {
    _module: NativePluginModule,
}

struct NativePluginModule {
    library: PlatformLibrary,
    #[cfg(all(unix, not(target_os = "macos")))]
    raw_handle: *mut c_void,
    entry_called: bool,
}

impl NativePluginModule {
    fn open(bundle_path: &Path) -> HostResult<Self> {
        let executable_path = crate::module::find_vst3_executable(bundle_path)?;

        // SAFETY: VST3 module loading is intentionally isolated to this crate
        // and ultimately to the worker process. The returned library is held
        // for the lifetime of all plugin objects created from its factory.
        let library = unsafe { PlatformLibrary::new(&executable_path) }
            .map_err(|error| HostError::ModuleLoadFailed(error.to_string()))?;

        #[cfg(target_os = "windows")]
        {
            Ok(Self {
                library,
                entry_called: false,
            })
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let raw_handle = library.into_raw();
            // SAFETY: `raw_handle` was just produced by `Library::into_raw`,
            // remains a valid dlopen handle, and is re-wrapped exactly once so
            // it will be dlclose'd by `PlatformLibrary` Drop.
            let library = unsafe { PlatformLibrary::from_raw(raw_handle) };
            Ok(Self {
                library,
                raw_handle,
                entry_called: false,
            })
        }
    }

    #[cfg(target_os = "windows")]
    fn call_entry(&mut self) -> HostResult<()> {
        if let Some(init) = self.optional_symbol::<InitDll>("InitDll") {
            // SAFETY: `InitDll` is the optional VST3 Windows module entry
            // point. It takes no arguments and returns a success flag.
            let ok = unsafe { init() };
            if !ok {
                return Err(HostError::FactoryCallFailed {
                    method: "InitDll",
                    result: i32::from(ok),
                });
            }
        }

        self.entry_called = true;
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn call_entry(&mut self) -> HostResult<()> {
        let entry = self.required_symbol::<ModuleEntry>("ModuleEntry")?;
        let _exit = self.required_symbol::<ModuleExit>("ModuleExit")?;

        // SAFETY: `ModuleEntry` is the required VST3 Linux/Unix entry point.
        // Steinberg hosts pass the raw dlopen handle; `self.raw_handle` is the
        // live handle backing `self.library`.
        let ok = unsafe { entry(self.raw_handle) };
        if !ok {
            return Err(HostError::FactoryCallFailed {
                method: "ModuleEntry",
                result: i32::from(ok),
            });
        }

        self.entry_called = true;
        Ok(())
    }

    fn plugin_factory(&self) -> HostResult<PluginFactory> {
        let get_plugin_factory = self.required_symbol::<GetPluginFactory>("GetPluginFactory")?;

        // SAFETY: The VST3 module entry point succeeded before this call, and
        // the symbol is the documented factory accessor. Null is handled below.
        let factory = unsafe { get_plugin_factory() };
        if factory.is_null() {
            return Err(HostError::FactoryReturnedNull);
        }

        // SAFETY: `factory` is an owned IPluginFactory pointer returned by the
        // module's `GetPluginFactory` after successful module entry.
        unsafe { PluginFactory::from_raw(factory) }
    }

    fn required_symbol<T>(&self, name: &'static str) -> HostResult<T>
    where
        T: Copy,
    {
        self.symbol(name)?.ok_or(HostError::MissingSymbol(name))
    }

    #[cfg(target_os = "windows")]
    fn optional_symbol<T>(&self, name: &'static str) -> Option<T>
    where
        T: Copy,
    {
        self.symbol(name).ok().flatten()
    }

    fn symbol<T>(&self, name: &'static str) -> HostResult<Option<T>>
    where
        T: Copy,
    {
        // SAFETY: The symbol name selects the expected VST3 entry-point type.
        // Returned function pointers are copied and only called while the
        // library is still owned by `self`.
        match unsafe { self.library.get::<T>(symbol_name(name)) } {
            Ok(symbol) => Ok(Some(*symbol)),
            Err(_) => Ok(None),
        }
    }
}

impl Drop for NativePluginModule {
    fn drop(&mut self) {
        if !self.entry_called {
            return;
        }

        #[cfg(target_os = "windows")]
        if let Some(exit) = self.optional_symbol::<ExitDll>("ExitDll") {
            // SAFETY: `ExitDll` is paired with a successful optional `InitDll`
            // call, or with a loaded module whose InitDll was absent.
            let _ = unsafe { exit() };
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        if let Ok(Some(exit)) = self.symbol::<ModuleExit>("ModuleExit") {
            // SAFETY: `ModuleExit` is paired with a successful `ModuleEntry`.
            let _ = unsafe { exit() };
        }
    }
}

fn symbol_name(name: &'static str) -> &'static [u8] {
    match name {
        "GetPluginFactory" => b"GetPluginFactory\0",
        "InitDll" => b"InitDll\0",
        "ExitDll" => b"ExitDll\0",
        "ModuleEntry" => b"ModuleEntry\0",
        "ModuleExit" => b"ModuleExit\0",
        _ => b"\0",
    }
}
