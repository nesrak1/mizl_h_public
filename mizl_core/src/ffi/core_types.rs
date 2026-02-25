use crate::ffi::core_framework::prelude::*;
use std::{ffi::c_void, ptr};

/// An object created by Box<> and with none of its fields exposed to the API.
/// These types of objects are expected to be mutable and live for a long time.
/// Can be "stolen" which means owmership returns to Rust (this library).
/// Stealing is useful when the user can allocate one of many objects to move
/// into another's constructor, such as a MemView.
pub struct OpaqueMFFI;
impl OpaqueMFFI {
    pub fn serialize(
        inp: *const c_void,
        vtable: Option<*const c_void>,
        free_ptr: extern "C" fn(obj: *mut u8),
    ) -> *mut u8 {
        let size = Self::calculate_size();

        const ALIGNMENT: usize = max_const_usize(WORD_SA, I32_SA);
        const DATA_OFF: usize = align_usize_fast_const::<WORD_SA>(PREFIX_HEADER_SZ);

        assert!(size < u32::MAX as usize, "allocated size must be fit in u32");

        unsafe {
            let layout = std::alloc::Layout::from_size_align_unchecked(size, ALIGNMENT);
            let ptr = std::alloc::alloc(layout);
            let ptrd = ptr.add(DATA_OFF);
            let mut ptrds = ptrd;

            *(ptrd.sub(I32_SZ * 3) as *mut u32) = (ALIGNMENT as u32) | HAS_FREE_POINTER_SUFFIX;
            *(ptrd.sub(I32_SZ * 2) as *mut u32) = (size - DATA_OFF - WORD_SZ) as u32;
            *(ptrd.sub(I32_SZ * 1) as *mut u32) = 0 as u32;

            // opaque data pointer
            *(ptrds as *mut *const c_void) = inp;

            // opaque vtable pointer
            ptrds = align_ptr_fast::<WORD_SA>(ptrds.add(WORD_SZ));
            *(ptrds as *mut *const c_void) = vtable.unwrap_or(std::ptr::null());

            // free function pointer (hidden)
            ptrds = align_ptr_fast::<WORD_SA>(ptrds.add(WORD_SZ));
            *(ptrds as *mut extern "C" fn(obj: *mut u8)) = free_ptr;

            return ptrd;
        }
    }

    pub fn calculate_size() -> usize {
        const DATA_OFF: usize = align_usize_fast_const::<WORD_SA>(PREFIX_HEADER_SZ);

        let mut size = DATA_OFF;

        // opaque data pointer
        size = align_usize_fast_const::<WORD_SA>(size);
        size += WORD_SA;

        // opaque vtable pointer
        size = align_usize_fast_const::<WORD_SA>(size);
        size += WORD_SA;

        // free function pointer (hidden)
        size = align_usize_fast_const::<WORD_SA>(size);
        size += WORD_SA;

        return size;
    }

    pub fn get_data_ptr(ptrd: *mut u8) -> *const c_void {
        unsafe {
            return *(ptrd as *mut *const c_void);
        }
    }

    pub fn get_vtable_ptr(ptrd: *mut u8) -> *const c_void {
        let mut ptrds = ptrd;
        unsafe {
            ptrds = align_ptr_fast::<WORD_SA>(ptrds.add(WORD_SZ));
            return *(ptrds as *mut *const c_void);
        }
    }

    pub fn get_free_ptr(ptrd: *mut u8) -> *const c_void {
        let mut ptrds = ptrd;
        unsafe {
            ptrds = align_ptr_fast::<WORD_SA>(ptrds.add(WORD_SZ));
            ptrds = align_ptr_fast::<WORD_SA>(ptrds.add(WORD_SZ));
            return *(ptrds as *mut *const c_void);
        }
    }

    /// Default free function for an opaque pheap object.
    pub extern "C" fn free_fn<T>(ptrd: *mut u8) {
        let obj = OpaqueMFFI::get_data_ptr(ptrd);
        if obj.is_null() {
            // object already freed or stolen
            return;
        }
        unsafe {
            _ = Box::from_raw(obj as *mut T);
        }
    }

    /// Steal ownership of the underlying object. Clears data pointer and free pointer.
    /// Box::from_raw gives the ownership back to Rust, so we don't call free.
    /// This could be an issue if there is further cleanup that needs to happen...
    pub fn steal<T>(ptrd: *mut u8) -> Result<Box<T>, ()> {
        let mut ptrds = ptrd;
        unsafe {
            let obj = *(ptrds as *const *const c_void);
            if obj.is_null() {
                // can't steal twice!
                return Err(());
            }

            *(ptrds.sub(I32_SZ * 3) as *mut u32) &= !HAS_FREE_POINTER_SUFFIX;
            *(ptrds as *mut *const c_void) = std::ptr::null();

            ptrds = align_ptr_fast::<WORD_SA>(ptrds.add(WORD_SZ));
            ptrds = align_ptr_fast::<WORD_SA>(ptrds.add(WORD_SZ));
            *(ptrds as *mut *const c_void) = std::ptr::null();

            Ok(Box::from_raw(obj as *mut T))
        }
    }
}

/// A string object. Not mutable.
pub struct StringFFI;
impl FfiSerializer for StringFFI {
    type Target = String;

    fn calculate_alignment() -> usize {
        I32_SA
    }

    fn calculate_base_size() -> usize {
        0
    }

    fn calculate_full_size(obj: &String) -> usize {
        I32_SZ + obj.len() + 1
    }

    fn has_dynamic_size() -> bool {
        true
    }

    fn has_var_length_field() -> bool {
        true
    }

    unsafe fn serialize(mut ptrd: *mut u8, obj: &String) -> *mut u8 {
        unsafe {
            // var length
            ptrd = align_ptr_fast::<I32_SA>(ptrd.add(I32_SZ));
            *(ptrd.sub(I32_SZ) as *mut u32) = obj.len() as u32;

            // string data
            ptr::copy_nonoverlapping(obj.as_ptr(), ptrd, obj.len());
            *ptrd.add(obj.len()) = 0;

            ptrd.add(obj.len() + 1)
        }
    }
}
impl FfiSerializeTrait for String {
    type Ffi = StringFFI;
}

// /////

pub trait FfiVecElement: Sized {
    fn element_alignment() -> usize;
    fn element_base_size() -> usize;
    fn element_full_size(obj: &Self) -> usize;
    fn element_has_dynamic_size() -> bool;
    fn element_is_inlined() -> bool; // does not use pointers. use for primtives.
    fn element_has_var_length_field() -> bool;
    unsafe fn serialize(ptrd: *mut u8, obj: &Self) -> *mut u8;
}

impl<T: FfiSerializeTrait> FfiVecElement for T {
    fn element_alignment() -> usize {
        T::Ffi::calculate_alignment()
    }
    fn element_base_size() -> usize {
        T::Ffi::calculate_base_size()
    }
    fn element_full_size(obj: &Self) -> usize {
        T::Ffi::calculate_full_size(obj)
    }
    fn element_has_dynamic_size() -> bool {
        T::Ffi::has_dynamic_size()
    }
    fn element_is_inlined() -> bool {
        false
    }
    fn element_has_var_length_field() -> bool {
        true
    }
    unsafe fn serialize(ptrd: *mut u8, obj: &Self) -> *mut u8 {
        unsafe { T::Ffi::serialize(ptrd, obj) }
    }
}

macro_rules! impl_ffi_element_primitive {
    ($ty:ty, $align:expr, $size:expr) => {
        impl FfiVecElement for $ty {
            fn element_alignment() -> usize {
                $align
            }
            fn element_base_size() -> usize {
                $size
            }
            fn element_full_size(_obj: &Self) -> usize {
                $size
            }
            fn element_has_dynamic_size() -> bool {
                false
            }
            fn element_is_inlined() -> bool {
                true
            }
            fn element_has_var_length_field() -> bool {
                false
            }
            unsafe fn serialize(ptrd: *mut u8, obj: &Self) -> *mut u8 {
                unsafe {
                    let ptr = align_ptr_fast::<{ $align }>(ptrd);
                    *(ptr as *mut $ty) = *obj;
                    ptr.add($size)
                }
            }
        }
    };
}

impl_ffi_element_primitive!(u8, I8_SA, I8_SZ);
impl_ffi_element_primitive!(i8, I8_SA, I8_SZ);
impl_ffi_element_primitive!(u16, I16_SA, I16_SZ);
impl_ffi_element_primitive!(i16, I16_SA, I16_SZ);
impl_ffi_element_primitive!(u32, I32_SA, I32_SZ);
impl_ffi_element_primitive!(i32, I32_SA, I32_SZ);
impl_ffi_element_primitive!(f32, I32_SA, I32_SZ);
impl_ffi_element_primitive!(u64, I64_SA, I64_SZ);
impl_ffi_element_primitive!(i64, I64_SA, I64_SZ);
impl_ffi_element_primitive!(f64, I64_SA, I64_SZ);
impl_ffi_element_primitive!(isize, WORD_SA, WORD_SZ);
impl_ffi_element_primitive!(usize, WORD_SA, WORD_SZ);

/// A vector of primitives or objects object. Not mutable.
pub struct VecFFI<T: FfiVecElement>(std::marker::PhantomData<T>);
impl<T: FfiVecElement> FfiSerializer for VecFFI<T> {
    type Target = Vec<T>;

    fn calculate_alignment() -> usize {
        // align could be smaller if using primitive type, but whatever, not that important
        max_const_usize(max_const_usize(I32_SA, WORD_SA), T::element_alignment())
    }

    fn calculate_base_size() -> usize {
        0
    }

    fn calculate_full_size(obj: &Vec<T>) -> usize {
        let mut size = 0;

        size = align_usize_fast_const::<I32_SA>(size + I32_SZ);

        // pointer array
        if !T::element_is_inlined() {
            size += obj.len() * WORD_SZ;
        }

        // element array
        if T::element_has_dynamic_size() {
            for elem in obj {
                size = align_usize_fast_var(size, T::element_alignment());
                if T::element_has_dynamic_size() {
                    size = align_usize_fast_var(size, I32_SA);
                }
                size += T::element_full_size(elem);
            }
        } else {
            size = align_usize_fast_var(size, T::element_alignment());
            size += T::element_base_size() * obj.len();
        }
        size
    }

    fn has_dynamic_size() -> bool {
        true
    }

    fn has_var_length_field() -> bool {
        true
    }

    unsafe fn serialize(mut ptrd: *mut u8, obj: &Vec<T>) -> *mut u8 {
        unsafe {
            // write var length
            ptrd = align_ptr_fast::<I32_SA>(ptrd.add(I32_SZ));
            *(ptrd.sub(I32_SZ) as *mut u32) = obj.len() as u32;

            if !T::element_is_inlined() {
                // also make a copy pointer for writing the pointer array and move to end
                let mut ptr_array = ptrd;
                ptrd = ptrd.add(obj.len() * WORD_SZ);

                // align and write each element
                for elem in obj {
                    ptrd = align_ptr_fast_var(ptrd, T::element_alignment());
                    let ptrd_elem_start = if T::element_has_var_length_field() {
                        align_ptr_fast::<I32_SA>(ptrd.add(I32_SZ))
                    } else {
                        ptrd
                    };
                    *(ptr_array as *mut *mut u8) = ptrd_elem_start;
                    ptr_array = ptr_array.add(WORD_SZ);
                    ptrd = T::serialize(ptrd, elem);
                }

                ptrd
            } else {
                // just start writing elements
                ptrd = align_ptr_fast_var(ptrd, T::element_alignment());
                for elem in obj {
                    ptrd = T::serialize(ptrd, elem);
                }

                ptrd
            }
        }
    }
}

impl<T: FfiVecElement> FfiSerializeTrait for Vec<T> {
    type Ffi = VecFFI<T>;
}

/// An error object. Not mutable.
pub struct ErrorFfi;
impl ErrorFfi {
    pub fn make_error(err_code: i32, err_str: Option<String>) -> *mut u8 {
        let err_tuple = (err_code, err_str);

        let size = ErrorFfi::calculate_full_size(&err_tuple);
        let align = ErrorFfi::calculate_alignment();

        let ptr = pheap_create(size, align, None);
        unsafe { ErrorFfi::serialize(ptr, &err_tuple) };
        ptr
    }
}
impl FfiSerializer for ErrorFfi {
    type Target = (i32, Option<String>);
    fn calculate_alignment() -> usize {
        // length field + character
        max_const_usize(I32_SA, I8_SA)
    }

    fn calculate_base_size() -> usize {
        0
    }

    fn calculate_full_size(inp: &(i32, Option<String>)) -> usize {
        let mut size = Self::calculate_base_size();

        // string data
        match &inp.1 {
            Some(s) => {
                size = align_usize_fast_const::<I8_SA>(size);
                size += s.len() + 1;
            }
            None => {
                // do nothing
            }
        };

        return size;
    }

    fn has_dynamic_size() -> bool {
        true
    }

    fn has_var_length_field() -> bool {
        true
    }

    unsafe fn serialize(ptrd: *mut u8, inp: &(i32, Option<String>)) -> *mut u8 {
        unsafe {
            let mut ptrd_dyn: *mut u8 = ptrd.add(Self::calculate_base_size());

            let err_code = inp.0;

            // pheap size (we assume we are the first and only serialized struct!)
            *(ptrd_dyn.sub(I32_SZ * 2) as *mut i32) = (-err_code - 1) as i32;

            match &inp.1 {
                Some(s) => {
                    // length
                    *(ptrd_dyn.sub(I32_SZ) as *mut u32) = s.len() as u32;

                    // string data
                    ptr::copy_nonoverlapping(s.as_ptr(), ptrd_dyn, s.len());
                    *(ptrd_dyn.add(s.len()) as *mut u8) = 0;
                    ptrd_dyn = ptrd_dyn.add(s.len() + 1);
                }
                None => {
                    // do nothing
                }
            };

            ptrd_dyn
        }
    }
}
