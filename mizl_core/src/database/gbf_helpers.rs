use crate::{
    consts::arch::Endianness,
    memory::memview::{MemView, MemViewError},
};

pub fn read_string(mv: &Box<dyn MemView>, at: &mut u64) -> Result<String, MemViewError> {
    let endian = Endianness::BigEndian; // always big endian

    let str_len = mv.read_i32(at, endian)?;
    if (str_len as u64 + *at) >= mv.max_address()? {
        return Err(MemViewError::EndOfStream);
    } else if str_len < 0 {
        let err_str = format!("invalid string length {}", str_len);
        return Err(MemViewError::generic_dynamic(err_str));
    }

    let mut str_bytes = vec![0u8; str_len as usize];
    mv.read_bytes(at, &mut str_bytes, str_len)?;
    match String::from_utf8(str_bytes) {
        Ok(v) => Ok(v),
        Err(_) => Err(MemViewError::generic_static("invalid utf-8 string read")),
    }
}

pub fn read_bytestring(mv: &Box<dyn MemView>, at: &mut u64) -> Result<Option<Vec<u8>>, MemViewError> {
    let endian = Endianness::BigEndian; // always big endian

    let bytes_len = mv.read_i32(at, endian)?;
    if bytes_len == -1 {
        return Ok(None);
    } else if (bytes_len as u64 + *at) >= mv.max_address()? {
        return Err(MemViewError::EndOfStream);
    } else if bytes_len < 0 {
        let err_str = format!("invalid string length {}", bytes_len);
        return Err(MemViewError::generic_dynamic(err_str));
    }

    let mut bytes = vec![0u8; bytes_len as usize];
    mv.read_bytes(at, &mut bytes, bytes_len)?;
    Ok(Some(bytes))
}
