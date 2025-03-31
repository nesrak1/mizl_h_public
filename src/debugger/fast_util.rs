use std::mem::transmute_copy;

pub fn read_swap_bytes<T>(data: &[u8], big_endian: bool) -> T
where
    T: Default + Copy,
{
    let type_size = std::mem::size_of::<T>();
    assert!(data.len() == type_size, "incorrect data size");
    assert!((type_size & 1) == 0, "odd size not supported");

    let swap = type_size > 1
        && if cfg!(target_endian = "big") {
            !big_endian
        } else {
            big_endian
        };

    // no swap needed
    if !swap {
        // safety: data is asserted for correct size for this type
        return unsafe { *(data.as_ptr() as *const T) };
    }

    // opto for common lengths
    match type_size {
        2 => {
            let mut val = unsafe { *(data.as_ptr() as *const u16) };
            val = u16::swap_bytes(val);

            // safety: data is asserted for correct size for this type
            unsafe {
                return transmute_copy::<u16, T>(&val);
            };
        }
        4 => {
            let mut val = unsafe { *(data.as_ptr() as *const u32) };
            val = u32::swap_bytes(val);

            // safety: data is asserted for correct size for this type
            unsafe {
                return transmute_copy::<u32, T>(&val);
            };
        }
        8 => {
            let mut val = unsafe { *(data.as_ptr() as *const u64) };
            val = u64::swap_bytes(val);

            // safety: data is asserted for correct size for this type
            unsafe {
                return transmute_copy::<u64, T>(&val);
            };
        }
        _ => {
            // we'll do it the slow way
            let mut tmp = vec![0u8; type_size];
            for i in 0..(type_size / 2) {
                tmp[i] = data[type_size - i];
                tmp[type_size - i] = data[i];
            }

            // safety: data is asserted for correct size for this type
            return unsafe { *(tmp.as_ptr() as *const T) };
        }
    }
}
