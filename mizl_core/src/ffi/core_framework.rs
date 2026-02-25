use std::ffi::c_void;

pub const I8_SZ: usize = std::mem::size_of::<i8>();
pub const I8_SA: usize = std::mem::align_of::<i8>();
pub const I16_SZ: usize = std::mem::size_of::<i16>();
pub const I16_SA: usize = std::mem::align_of::<i16>();
pub const I32_SZ: usize = std::mem::size_of::<i32>();
pub const I32_SA: usize = std::mem::align_of::<i32>();
pub const I64_SZ: usize = std::mem::size_of::<i64>();
pub const I64_SA: usize = std::mem::align_of::<i64>();
pub const WORD_SZ: usize = std::mem::size_of::<usize>();
pub const WORD_SA: usize = std::mem::align_of::<usize>();

pub mod prelude {
    pub use crate::ffi::core_framework::{
        FfiSerializeTrait, FfiSerializer, HAS_FREE_POINTER_SUFFIX, I8_SA, I8_SZ, I16_SA, I16_SZ, I32_SA, I32_SZ,
        I64_SA, I64_SZ, PREFIX_HEADER_SZ, WORD_SA, WORD_SZ, align_ptr_fast, align_ptr_fast_var, align_usize_fast_const,
        align_usize_fast_var, max_const_usize, pheap_alloc, pheap_create,
    };
    pub use mizl_pm::FfiSerialize;
}

// /////

pub const fn align_usize_fast_const<const TS: usize>(sz: usize) -> usize {
    assert!(TS > 0 && (TS & (TS - 1)) == 0, "alignment must be a power of two");
    (sz + (TS - 1)) & !(TS - 1)
}

pub fn align_usize_fast_var(sz: usize, align: usize) -> usize {
    debug_assert!(
        align > 0 && (align & (align - 1)) == 0,
        "alignment must be a power of two"
    );
    (sz + (align - 1)) & !(align - 1)
}

pub fn align_ptr_fast<const TS: usize>(ptr: *mut u8) -> *mut u8 {
    align_usize_fast_const::<TS>(ptr as usize) as *mut u8
}

pub fn align_ptr_fast_var(ptr: *mut u8, align: usize) -> *mut u8 {
    align_usize_fast_var(ptr as usize, align) as *mut u8
}

pub const fn max_const_usize(a: usize, b: usize) -> usize {
    return [a, b][(a < b) as usize];
}

// /////

pub trait FfiSerializeTrait: Sized {
    type Ffi: FfiSerializer<Target = Self>;
}

pub trait FfiSerializer {
    type Target;

    fn calculate_alignment() -> usize; // includes alignment of variable length field
    fn calculate_base_size() -> usize; // does not include length of variable length field
    fn calculate_full_size(obj: &Self::Target) -> usize;
    fn has_dynamic_size() -> bool;
    fn has_var_length_field() -> bool;

    unsafe fn serialize(ptrd: *mut u8, obj: &Self::Target) -> *mut u8;
}

// /////

pub const PREFIX_HEADER_SZ: usize = 12;
pub const HAS_FREE_POINTER_SUFFIX: u32 = 1 << 31;

/// Allocate blank pheap with a size, alignment, and possible free pointer.
pub fn pheap_create(size: usize, align: usize, free_ptr: Option<extern "C" fn(obj: *const c_void)>) -> *mut u8 {
    let data_off = align_usize_fast_var(PREFIX_HEADER_SZ, align);
    let has_free = free_ptr.is_some();
    let flags = if has_free { HAS_FREE_POINTER_SUFFIX } else { 0 };
    let hidden = if has_free { WORD_SZ } else { 0 };

    // println!(
    //     "[PHCREATE] alignment: {align}, has_free: {has_free}, size: {size}, size_tot: {}",
    //     size + data_off + hidden
    // );

    unsafe {
        let layout = std::alloc::Layout::from_size_align_unchecked(size + data_off + hidden, align);
        let ptr = std::alloc::alloc(layout);
        let ptrd = ptr.add(data_off);

        *(ptrd.sub(12) as *mut u32) = (align as u32) | flags;
        *(ptrd.sub(8) as *mut u32) = size as u32;
        *(ptrd.sub(4) as *mut u32) = 0u32; // var-length field, zeroed

        if let Some(fp) = free_ptr {
            // store the optional free function ptr in the hidden suffix after the data
            *(ptrd.add(size - data_off - hidden) as *mut usize) = fp as usize;
        }

        ptrd
    }
}

pub fn pheap_alloc<T: FfiSerializeTrait>(obj: &T, free_ptr: Option<extern "C" fn(obj: *const c_void)>) -> *mut u8 {
    let mut size = T::Ffi::calculate_full_size(obj);
    let align = T::Ffi::calculate_alignment();
    let has_var_length_field = T::Ffi::has_var_length_field();

    if has_var_length_field {
        size -= 4; // remove variable length field from full size
    }

    // println!("[PHALLOC] alignment: {align}, size: {size}");

    let ptr = pheap_create(size, align, free_ptr);

    let ptr_serialize = if has_var_length_field {
        unsafe { ptr.sub(4) } // start four bytes backwards (serialize will jump ahead four again)
    } else {
        ptr
    };

    unsafe { T::Ffi::serialize(ptr_serialize, obj) };
    ptr
}

/// Free pheap and call the free pointer if it is set.
#[unsafe(no_mangle)]
pub extern "C" fn pheap_free(ptrd: *mut u8) {
    unsafe {
        let alignment_enc = *(ptrd.sub(12) as *mut u32);
        let alignment = (alignment_enc & !HAS_FREE_POINTER_SUFFIX) as usize;
        let has_free = (alignment_enc & HAS_FREE_POINTER_SUFFIX) != 0;

        let data_off = align_usize_fast_var(PREFIX_HEADER_SZ, alignment);

        let size_enc = *(ptrd.sub(8) as *mut i32);
        let is_error = size_enc < 0;
        let size = if is_error {
            // use error string length to calculate size
            let var_len = *(ptrd.sub(4) as *mut u32) as usize;
            var_len + 1
        } else {
            // use size as normal
            size_enc as u32 as usize
        };

        // println!("[PHFREE] alignment: {alignment}, has_free: {has_free}, size: {size}, is_error: {is_error}");

        if has_free {
            let ptrf = align_ptr_fast::<WORD_SZ>(ptrd.add(size));
            let free_ptr = *(ptrf as *const *const c_void);
            let free_fn: unsafe extern "C" fn(*const u8) = std::mem::transmute_copy(&free_ptr);
            free_fn(ptrd);
        }

        let real_size = size + data_off;

        let ptr = ptrd.sub(data_off);
        let layout = std::alloc::Layout::from_size_align_unchecked(real_size, alignment);
        std::alloc::dealloc(ptr, layout);
    }
}
