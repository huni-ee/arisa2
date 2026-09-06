use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_void},
    ptr,
};

use jni::sys::{JNI_OK, JNI_VERSION_1_6};

const RTLD_NOW: c_int = 2;
const RTLD_GLOBAL: c_int = 0x100;

#[repr(C)]
struct JniInvocation {
    _private: [u8; 0],
}

type CreateJavaVm = unsafe extern "C" fn(
    *mut *mut jni::sys::JavaVM,
    *mut *mut jni::sys::JNIEnv,
    *mut jni::sys::JavaVMInitArgs,
) -> i32;

type InvocationCreate = unsafe extern "C" fn() -> *mut JniInvocation;
type InvocationInit = unsafe extern "C" fn(*mut JniInvocation, *const c_char) -> c_int;
type RegisterFrameworkNatives = unsafe extern "C" fn(*mut jni::sys::JNIEnv) -> i32;
type RegisterFrameworkNativesLegacy =
    unsafe extern "C" fn(*mut jni::sys::JNIEnv, *mut c_void) -> i32;

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
}

unsafe fn dl_error() -> String {
    let error = unsafe { dlerror() };

    if error.is_null() {
        "unknown dl error".to_string()
    } else {
        unsafe { CStr::from_ptr(error).to_string_lossy().into_owned() }
    }
}

unsafe fn open_library(name: &str, flags: c_int) -> Result<*mut c_void, String> {
    let name = CString::new(name).map_err(|_| format!("library name contains NUL: {name:?}"))?;

    let handle = unsafe { dlopen(name.as_ptr(), flags) };

    if handle.is_null() {
        Err(unsafe { dl_error() })
    } else {
        Ok(handle)
    }
}

unsafe fn optional_library(name: &str, flags: c_int) -> Option<*mut c_void> {
    unsafe { open_library(name, flags).ok() }
}

unsafe fn symbol<T>(handle: *mut c_void, name: &str) -> Result<T, String> {
    let name = CString::new(name).map_err(|_| format!("invalid symbol name: {name:?}"))?;

    let value = unsafe { dlsym(handle, name.as_ptr()) };
    if value.is_null() {
        Err(unsafe { dl_error() })
    } else {
        Ok(unsafe { std::mem::transmute_copy(&value) })
    }
}

unsafe fn optional_symbol<T>(handle: *mut c_void, name: &str) -> Option<T> {
    unsafe { symbol(handle, name).ok() }
}

unsafe fn invoke_create_vm(
    create_vm: CreateJavaVm,
) -> Result<(*mut jni::sys::JavaVM, *mut jni::sys::JNIEnv), String> {
    let mut arguments = jni::sys::JavaVMInitArgs {
        version: JNI_VERSION_1_6,
        nOptions: 0,
        options: ptr::null_mut(),
        ignoreUnrecognized: false,
    };

    let mut raw_vm: *mut jni::sys::JavaVM = ptr::null_mut();
    let mut raw_env: *mut jni::sys::JNIEnv = ptr::null_mut();

    let status = unsafe { create_vm(&mut raw_vm, &mut raw_env, &mut arguments) };

    if status != JNI_OK {
        return Err(format!("JNI_CreateJavaVM failed: {status}"));
    }

    if raw_vm.is_null() {
        return Err("JNI_CreateJavaVM returned a null JavaVM".to_string());
    }

    if raw_env.is_null() {
        return Err("JNI_CreateJavaVM returned a null JNIEnv".to_string());
    }

    Ok((raw_vm, raw_env))
}

unsafe fn create_vm_with_nativehelper(
    nativehelper: *mut c_void,
) -> Result<(*mut jni::sys::JavaVM, *mut jni::sys::JNIEnv), String> {
    let create_invocation: InvocationCreate =
        unsafe { symbol(nativehelper, "JniInvocationCreate") }?;

    let init_invocation: InvocationInit = unsafe { symbol(nativehelper, "JniInvocationInit") }?;

    let invocation = unsafe { create_invocation() };

    if invocation.is_null() {
        return Err("JniInvocationCreate returned null".to_string());
    }

    let initialized = unsafe { init_invocation(invocation, ptr::null()) };
    if initialized == 0 {
        return Err("JniInvocationInit failed".to_string());
    }

    let create_vm: CreateJavaVm = unsafe { symbol(nativehelper, "JNI_CreateJavaVM") }?;

    unsafe { invoke_create_vm(create_vm) }
}

unsafe fn create_vm_from_runtime(
    runtime_name: &str,
) -> Result<(*mut jni::sys::JavaVM, *mut jni::sys::JNIEnv), String> {
    let runtime = unsafe { open_library(runtime_name, RTLD_NOW | RTLD_GLOBAL) }
        .map_err(|error| format!("cannot open {runtime_name}: {error}"))?;

    let create_vm: CreateJavaVm = unsafe { symbol(runtime, "JNI_CreateJavaVM") }
        .map_err(|error| format!("{runtime_name} has no usable JNI_CreateJavaVM: {error}"))?;

    unsafe { invoke_create_vm(create_vm) }
}

unsafe fn create_vm_legacy() -> Result<(*mut jni::sys::JavaVM, *mut jni::sys::JNIEnv), String> {
    const RUNTIMES: &[&str] = &["libart.so", "libdvm.so"];

    let mut errors = Vec::new();

    for runtime in RUNTIMES {
        match unsafe { create_vm_from_runtime(runtime) } {
            Ok(vm) => {
                return Ok(vm);
            }

            Err(error) => {
                errors.push(error);
            }
        }
    }

    Err(format!(
        "no usable Android Java runtime found: {}",
        errors.join("; ")
    ))
}

unsafe fn try_create_vm_with_nativehelper()
-> Result<Option<(*mut jni::sys::JavaVM, *mut jni::sys::JNIEnv)>, String> {
    let Some(nativehelper) =
        (unsafe { optional_library("libnativehelper.so", RTLD_NOW | RTLD_GLOBAL) })
    else {
        return Ok(None);
    };

    let has_create =
        unsafe { optional_symbol::<InvocationCreate>(nativehelper, "JniInvocationCreate") }
            .is_some();

    let has_init =
        unsafe { optional_symbol::<InvocationInit>(nativehelper, "JniInvocationInit") }.is_some();

    let has_create_vm =
        unsafe { optional_symbol::<CreateJavaVm>(nativehelper, "JNI_CreateJavaVM") }.is_some();

    if !has_create || !has_init || !has_create_vm {
        return Ok(None);
    }

    unsafe { create_vm_with_nativehelper(nativehelper).map(Some) }
}

unsafe fn register_framework_natives(
    android_runtime: *mut c_void,
    env: *mut jni::sys::JNIEnv,
) -> Result<(), String> {
    if let Some(register) = unsafe {
        optional_symbol::<RegisterFrameworkNatives>(android_runtime, "registerFrameworkNatives")
    } {
        let result = unsafe { register(env) };

        if result != JNI_OK {
            return Err(format!("registerFrameworkNatives failed: {result}"));
        }

        return Ok(());
    }

    if let Some(register) = unsafe {
        optional_symbol::<RegisterFrameworkNativesLegacy>(
            android_runtime,
            "Java_com_android_internal_util_WithFramework_registerNatives",
        )
    } {
        let result = unsafe { register(env, ptr::null_mut()) };

        if result != JNI_OK {
            return Err(format!("WithFramework.registerNatives failed: {result}"));
        }

        return Ok(());
    }

    Err("no framework native registration function found".to_string())
}

pub unsafe fn create_android_vm() -> Result<jni::JavaVM, String> {
    let _ = unsafe { optional_library("libsigchain.so", RTLD_NOW | RTLD_GLOBAL) };

    let android_runtime = unsafe { open_library("libandroid_runtime.so", RTLD_NOW | RTLD_GLOBAL) }
        .map_err(|error| format!("cannot open libandroid_runtime.so: {error}"))?;

    let (raw_vm, raw_env) = match unsafe { try_create_vm_with_nativehelper() } {
        Ok(Some(vm)) => vm,

        Ok(None) => unsafe { create_vm_legacy() }?,

        Err(nativehelper_error) => {
            return Err(format!(
                "JniInvocation initialization failed: \
                     {nativehelper_error}"
            ));
        }
    };

    unsafe { register_framework_natives(android_runtime, raw_env) }?;

    Ok(unsafe { jni::JavaVM::from_raw(raw_vm) })
}
