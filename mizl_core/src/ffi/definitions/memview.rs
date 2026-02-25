use crate::{
    ffi::core_types::{ErrorFfi, OpaqueMFFI},
    memory::memview::{MemView, MemViewError, StaticMemView},
};
use std::ffi::{CStr, c_char, c_void};

pub fn mem_view_error_ffi(error_opt: Option<&MemViewError>) -> *mut u8 {
    match error_opt {
        Some(error) => {
            let error_code = match error {
                MemViewError::EndOfStream => 0,
                MemViewError::ReadAccessDenied => 1,
                MemViewError::WriteAccessDenied => 2,
                MemViewError::NotLoaded => 3,
                MemViewError::InvalidParameter => 4,
                MemViewError::Generic(_) => 5,
            };
            let error_str: String = error.to_string();
            let error_mffi_ptr = ErrorFfi::make_error(error_code, Some(error_str));

            error_mffi_ptr
        }
        None => {
            let error_code = i32::MAX;
            let error_mffi_ptr = ErrorFfi::make_error(error_code, None);

            error_mffi_ptr
        }
    }
}

/// Assign the err out parameter but return nothing
pub fn mem_view_error_ret(err: *mut *const u8, error_opt: Option<&MemViewError>) {
    unsafe {
        *err = mem_view_error_ffi(error_opt);
    }
}

/// Return the function's default primitive value and assign the err out parameter
pub fn mem_view_error_dret<T: Default>(err: *mut *const u8, error_opt: Option<&MemViewError>) -> T {
    unsafe {
        *err = mem_view_error_ffi(error_opt);
    }
    T::default()
}

/// Return a null pointer value and assign the err out parameter
pub fn mem_view_error_pret(err: *mut *const u8, error_opt: Option<&MemViewError>) -> *mut u8 {
    unsafe {
        *err = mem_view_error_ffi(error_opt);
    }
    std::ptr::null_mut()
}

/// Return a null pointer value and assign the err out parameter
pub fn mem_view_error_cpret<T>(err: *mut *const u8, error_opt: Option<&MemViewError>) -> *const T {
    unsafe {
        *err = mem_view_error_ffi(error_opt);
    }
    std::ptr::null() as *const T
}

// ///////

pub struct MemViewFfiWrap {
    pub obj: Box<dyn MemView>,
}

#[repr(C)]
pub struct MemViewVTable {
    // todo
    pub steal: fn(*const c_void) -> Result<Box<dyn MemView>, ()>,
}

// #-class MemView

static STATIC_MEM_VIEW_VTABLE: MemViewVTable = MemViewVTable {
    // todo
    steal: static_mem_view_steal,
};

#[unsafe(no_mangle)]
pub extern "C" fn static_mem_view_from_file(path: *const c_char, err: *mut *const u8) -> *mut u8 {
    let path_str = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(v) => v,
        Err(_) => return mem_view_error_pret(err, Some(&MemViewError::InvalidParameter)),
    };

    match std::fs::read(path_str) {
        Ok(file_data) => {
            let static_mem_view = StaticMemView::new(file_data);
            let static_mem_view_box = Box::new(static_mem_view);
            let static_mem_view_box_ptr = Box::into_raw(static_mem_view_box);

            let static_mem_view_ptr = OpaqueMFFI::serialize(
                static_mem_view_box_ptr as *const c_void,
                Some(&STATIC_MEM_VIEW_VTABLE as *const MemViewVTable as *const c_void),
                OpaqueMFFI::free_fn::<StaticMemView>,
            );

            static_mem_view_ptr
        }
        Err(_) => return mem_view_error_pret(err, Some(&MemViewError::ReadAccessDenied)),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn static_mem_view_from_data(data: *const u8, len: u64) -> *mut u8 {
    let slice = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let file_data = slice.to_vec();

    let static_mem_view = StaticMemView::new(file_data);
    let static_mem_view_box = Box::new(static_mem_view);
    let static_mem_view_box_ptr = Box::into_raw(static_mem_view_box);

    let static_mem_view_ptr = OpaqueMFFI::serialize(
        static_mem_view_box_ptr as *const c_void,
        Some(&STATIC_MEM_VIEW_VTABLE as *const MemViewVTable as *const c_void),
        OpaqueMFFI::free_fn::<StaticMemView>,
    );

    static_mem_view_ptr
}

pub fn static_mem_view_steal(obj: *const c_void) -> Result<Box<dyn MemView>, ()> {
    let stolen_obj = OpaqueMFFI::steal::<StaticMemView>(obj as *mut u8)?;
    Ok(stolen_obj as Box<dyn MemView>)
}
