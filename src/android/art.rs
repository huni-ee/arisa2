use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_void},
    ptr,
};

use jni::sys::{JNI_OK, JNI_VERSION_1_6};

const RTLD_NOW: c_int = 2;
const JNI_INVOCATION_CPP_OBJECT_BYTES: usize = 256;

#[repr(C)]
struct JniInvocation {
    _private: [u8; 0],
}

type CreateJavaVm = unsafe extern "C" fn(
    *mut *mut jni::sys::JavaVM,
    *mut *mut jni::sys::JNIEnv,
    *mut jni::sys::JavaVMInitArgs,
) -> i32;
type RegisterFrameworkNatives = unsafe extern "C" fn(*mut jni::sys::JNIEnv) -> i32;
type RegisterFrameworkNativesLegacy =
    unsafe extern "C" fn(*mut jni::sys::JNIEnv, *mut c_void) -> i32;
type InvocationCreate = unsafe extern "C" fn() -> *mut JniInvocation;
type InvocationInit = unsafe extern "C" fn(*mut JniInvocation, *const c_char) -> c_int;
type InvocationCtor = unsafe extern "C" fn(*mut JniInvocation);
type InvocationInitMethod = unsafe extern "C" fn(*mut JniInvocation, *const c_char) -> bool;

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
    fn malloc(size: usize) -> *mut c_void;
}

#[unsafe(no_mangle)]
pub extern "C" fn SetSpecialSignalHandlerFn(_: c_int, _: *mut c_void) {}

#[unsafe(no_mangle)]
pub extern "C" fn GetSpecialSignalHandlerFn(_: c_int) -> *mut c_void {
    ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn EnsureFrontOfChain(_: c_int) {}

#[unsafe(no_mangle)]
pub extern "C" fn InitializeSignalChain() {}

#[unsafe(no_mangle)]
pub extern "C" fn AddSpecialSignalHandlerFn(_: c_int, _: *mut c_void) {}

#[unsafe(no_mangle)]
pub extern "C" fn RemoveSpecialSignalHandlerFn(_: c_int, _: *mut c_void) {}

unsafe fn dl_error() -> String {
    let error = unsafe { dlerror() };
    if error.is_null() {
        "unknown dl error".to_string()
    } else {
        unsafe { CStr::from_ptr(error).to_string_lossy().into_owned() }
    }
}

unsafe fn open_library(name: &str) -> Result<*mut c_void, String> {
    let name = CString::new(name).expect("library name cannot contain NUL");
    let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW) };
    if handle.is_null() {
        Err(unsafe { dl_error() })
    } else {
        Ok(handle)
    }
}

unsafe fn symbol<T>(handle: *mut c_void, name: &str) -> Result<T, String> {
    let name = CString::new(name).expect("symbol name cannot contain NUL");
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

unsafe fn initialize_invocation(handle: *mut c_void, runtime: &CString) -> Result<(), String> {
    let create: Option<InvocationCreate> =
        unsafe { optional_symbol(handle, "JniInvocationCreate") };
    let init: Option<InvocationInit> = unsafe { optional_symbol(handle, "JniInvocationInit") };

    if let (Some(create), Some(init)) = (create, init) {
        let invocation = unsafe { create() };
        if invocation.is_null() || unsafe { init(invocation, runtime.as_ptr()) } == 0 {
            return Err("JniInvocation initialization failed".to_string());
        }
        return Ok(());
    }

    let constructor: InvocationCtor = unsafe { symbol(handle, "_ZN13JniInvocationC1Ev") }?;
    let init: InvocationInitMethod = unsafe { symbol(handle, "_ZN13JniInvocation4InitEPKc") }?;
    let invocation = unsafe { malloc(JNI_INVOCATION_CPP_OBJECT_BYTES) as *mut JniInvocation };
    if invocation.is_null() {
        return Err("JniInvocation allocation failed".to_string());
    }
    unsafe { constructor(invocation) };
    if !unsafe { init(invocation, ptr::null()) } && !unsafe { init(invocation, runtime.as_ptr()) } {
        return Err("JniInvocation initialization failed".to_string());
    }
    Ok(())
}

pub unsafe fn create_android_vm() -> Result<jni::JavaVM, String> {
    let handle = unsafe { open_library("libandroid_runtime.so") }
        .map_err(|error| format!("cannot open Android runtime: {error}"))?;
    let runtime = CString::new("libandroid_runtime.so").unwrap();
    unsafe { initialize_invocation(handle, &runtime) }?;

    let create: CreateJavaVm = unsafe { symbol(handle, "JNI_CreateJavaVM") }?;
    let mut arguments = jni::sys::JavaVMInitArgs {
        version: JNI_VERSION_1_6,
        nOptions: 0,
        options: ptr::null_mut(),
        ignoreUnrecognized: false,
    };
    let mut raw_vm = ptr::null_mut();
    let mut raw_env = ptr::null_mut();
    let status = unsafe { create(&mut raw_vm, &mut raw_env, &mut arguments) };
    if status != JNI_OK || raw_vm.is_null() || raw_env.is_null() {
        return Err(format!("JNI_CreateJavaVM failed: {status}"));
    }

    let register_result = if let Ok(register) =
        unsafe { symbol::<RegisterFrameworkNatives>(handle, "registerFrameworkNatives") }
    {
        unsafe { register(raw_env) }
    } else {
        let register: RegisterFrameworkNativesLegacy = unsafe {
            symbol(
                handle,
                "Java_com_android_internal_util_WithFramework_registerNatives",
            )
        }?;
        unsafe { register(raw_env, ptr::null_mut()) }
    };
    if register_result != JNI_OK {
        return Err(format!(
            "registerFrameworkNatives failed: {register_result}"
        ));
    }

    Ok(unsafe { jni::JavaVM::from_raw(raw_vm) })
}
