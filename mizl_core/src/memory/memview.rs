use crate::consts::arch::Endianness;
use std::{borrow::Cow, fmt};

#[derive(Debug)]
pub enum MemViewError {
    EndOfStream,
    ReadAccessDenied,
    WriteAccessDenied,
    NotLoaded,
    InvalidParameter,
    Generic(Cow<'static, str>),
}

impl MemViewError {
    pub fn generic_static(msg: &'static str) -> MemViewError {
        MemViewError::Generic(Cow::Borrowed(msg))
    }

    pub fn generic_dynamic(msg: String) -> MemViewError {
        MemViewError::Generic(Cow::Owned(msg))
    }
}

impl fmt::Display for MemViewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MemViewError::EndOfStream => write!(f, "address was further than stream length"),
            MemViewError::ReadAccessDenied => write!(f, "data was unable to be read"),
            MemViewError::WriteAccessDenied => write!(f, "data was unable to be written"),
            MemViewError::NotLoaded => write!(f, "memory is not yet loaded or was recently unloaded"),
            MemViewError::InvalidParameter => write!(f, "bad parameter"),
            MemViewError::Generic(s) => write!(f, "{}", s),
        }
    }
}

// we use u64 instead of usize in order to allow 32-bit devices
// to debug 64-bit remote devices. of course, this means anything
// larger than 64-bit isn't supported at all, but I doubt we will
// run into many 128-bit addressed devices anytime soon...
pub trait MemView {
    fn read_bytes(&self, addr: &mut u64, out_data: &mut [u8], count: i32) -> Result<(), MemViewError>;
    fn write_bytes(&mut self, addr: &mut u64, value: &[u8]) -> Result<(), MemViewError>;

    // always returns one byte after the last byte.
    // ex: if last byte is at 0xf, this should be 0x10
    //     if the memory view is empty, this should be 0x0
    //     if all addresses are accessible, this returns u64::MAX
    // note that this means a max address of u64::MAX - 1
    // can't be returned since that would also be u64::MAX.
    fn max_address(&self) -> Result<u64, MemViewError>;

    fn can_read_while_running(&self) -> bool;
    fn can_write_while_running(&self) -> bool;

    fn read_u8(&self, addr: &mut u64) -> Result<u8, MemViewError> {
        let mut bytes = [0u8; 1];
        self.read_bytes(addr, &mut bytes, 1)?;
        Ok(bytes[0])
    }

    fn read_u16(&self, addr: &mut u64, endian: Endianness) -> Result<u16, MemViewError> {
        let mut bytes = [0u8; 2];
        self.read_bytes(addr, &mut bytes, 2)?;
        if endian == Endianness::LittleEndian {
            Ok(u16::from_le_bytes(bytes))
        } else {
            Ok(u16::from_be_bytes(bytes))
        }
    }

    fn read_u32(&self, addr: &mut u64, endian: Endianness) -> Result<u32, MemViewError> {
        let mut bytes = [0u8; 4];
        self.read_bytes(addr, &mut bytes, 4)?;
        if endian == Endianness::LittleEndian {
            Ok(u32::from_le_bytes(bytes))
        } else {
            Ok(u32::from_be_bytes(bytes))
        }
    }

    fn read_u64(&self, addr: &mut u64, endian: Endianness) -> Result<u64, MemViewError> {
        let mut bytes = [0u8; 8];
        self.read_bytes(addr, &mut bytes, 8)?;
        if endian == Endianness::LittleEndian {
            Ok(u64::from_le_bytes(bytes))
        } else {
            Ok(u64::from_be_bytes(bytes))
        }
    }

    fn read_i8(&self, addr: &mut u64) -> Result<i8, MemViewError> {
        let mut bytes = [0u8; 1];
        self.read_bytes(addr, &mut bytes, 1)?;
        Ok(bytes[0] as i8)
    }

    fn read_i16(&self, addr: &mut u64, endian: Endianness) -> Result<i16, MemViewError> {
        let mut bytes = [0u8; 2];
        self.read_bytes(addr, &mut bytes, 2)?;
        if endian == Endianness::LittleEndian {
            Ok(i16::from_le_bytes(bytes))
        } else {
            Ok(i16::from_be_bytes(bytes))
        }
    }

    fn read_i32(&self, addr: &mut u64, endian: Endianness) -> Result<i32, MemViewError> {
        let mut bytes = [0u8; 4];
        self.read_bytes(addr, &mut bytes, 4)?;
        if endian == Endianness::LittleEndian {
            Ok(i32::from_le_bytes(bytes))
        } else {
            Ok(i32::from_be_bytes(bytes))
        }
    }

    fn read_i64(&self, addr: &mut u64, endian: Endianness) -> Result<i64, MemViewError> {
        let mut bytes = [0u8; 8];
        self.read_bytes(addr, &mut bytes, 8)?;
        if endian == Endianness::LittleEndian {
            Ok(i64::from_le_bytes(bytes))
        } else {
            Ok(i64::from_be_bytes(bytes))
        }
    }

    fn read_f32(&mut self, addr: &mut u64, endian: Endianness) -> Result<f32, MemViewError> {
        let mut bytes = [0u8; 4];
        self.read_bytes(addr, &mut bytes, 4)?;
        if endian == Endianness::LittleEndian {
            Ok(f32::from_le_bytes(bytes))
        } else {
            Ok(f32::from_be_bytes(bytes))
        }
    }

    fn read_f64(&mut self, addr: &mut u64, endian: Endianness) -> Result<f64, MemViewError> {
        let mut bytes = [0u8; 8];
        self.read_bytes(addr, &mut bytes, 8)?;
        if endian == Endianness::LittleEndian {
            Ok(f64::from_le_bytes(bytes))
        } else {
            Ok(f64::from_be_bytes(bytes))
        }
    }

    fn write_u8(&mut self, addr: &mut u64, value: u8) -> Result<(), MemViewError> {
        let v = [value];
        self.write_bytes(addr, &v)
    }

    fn write_u16(&mut self, addr: &mut u64, value: u16, endian: Endianness) -> Result<(), MemViewError> {
        let v = if endian == Endianness::LittleEndian {
            u16::to_be_bytes(value)
        } else {
            u16::to_le_bytes(value)
        };
        self.write_bytes(addr, &v)
    }

    fn write_u32(&mut self, addr: &mut u64, value: u32, endian: Endianness) -> Result<(), MemViewError> {
        let v = if endian == Endianness::LittleEndian {
            u32::to_be_bytes(value)
        } else {
            u32::to_le_bytes(value)
        };
        self.write_bytes(addr, &v)
    }

    fn write_u64(&mut self, addr: &mut u64, value: u64, endian: Endianness) -> Result<(), MemViewError> {
        let v = if endian == Endianness::LittleEndian {
            u64::to_be_bytes(value)
        } else {
            u64::to_le_bytes(value)
        };
        self.write_bytes(addr, &v)
    }

    fn write_i8(&mut self, addr: &mut u64, value: i8) -> Result<(), MemViewError> {
        let v = [value as u8];
        self.write_bytes(addr, &v)
    }

    fn write_i16(&mut self, addr: &mut u64, value: i16, endian: Endianness) -> Result<(), MemViewError> {
        let v = if endian == Endianness::LittleEndian {
            i16::to_be_bytes(value)
        } else {
            i16::to_le_bytes(value)
        };
        self.write_bytes(addr, &v)
    }

    fn write_i32(&mut self, addr: &mut u64, value: i32, endian: Endianness) -> Result<(), MemViewError> {
        let v = if endian == Endianness::LittleEndian {
            i32::to_be_bytes(value)
        } else {
            i32::to_le_bytes(value)
        };
        self.write_bytes(addr, &v)
    }

    fn write_i64(&mut self, addr: &mut u64, value: i64, endian: Endianness) -> Result<(), MemViewError> {
        let v = if endian == Endianness::LittleEndian {
            i64::to_be_bytes(value)
        } else {
            i64::to_le_bytes(value)
        };
        self.write_bytes(addr, &v)
    }

    fn write_f32(&mut self, addr: &mut u64, value: f32, endian: Endianness) -> Result<(), MemViewError> {
        let v = if endian == Endianness::LittleEndian {
            f32::to_be_bytes(value)
        } else {
            f32::to_le_bytes(value)
        };
        self.write_bytes(addr, &v)
    }

    fn write_f64(&mut self, addr: &mut u64, value: f64, endian: Endianness) -> Result<(), MemViewError> {
        let v = if endian == Endianness::LittleEndian {
            f64::to_be_bytes(value)
        } else {
            f64::to_le_bytes(value)
        };
        self.write_bytes(addr, &v)
    }
}

pub struct StaticMemView {
    data: Vec<u8>,
}

impl StaticMemView {
    pub fn new(data: Vec<u8>) -> StaticMemView {
        StaticMemView { data }
    }
}

impl MemView for StaticMemView {
    fn read_bytes(&self, addr: &mut u64, out_data: &mut [u8], count: i32) -> Result<(), MemViewError> {
        let data_len = self.data.len();
        let addr_val = *addr as usize;
        let addr_end_val = addr_val + count as usize;
        if addr_end_val >= data_len {
            return Err(MemViewError::EndOfStream);
        }

        *addr += count as u64;
        out_data.clone_from_slice(&self.data[addr_val as usize..addr_end_val]);
        Ok(())
    }

    fn write_bytes(&mut self, addr: &mut u64, value: &[u8]) -> Result<(), MemViewError> {
        let data_len = self.data.len();
        let count = value.len();
        let addr_val = *addr as usize;
        let addr_end_val = addr_val + count as usize;
        if addr_end_val >= data_len {
            return Err(MemViewError::EndOfStream);
        }

        *addr += count as u64;
        self.data.splice(addr_val..addr_end_val, value.iter().cloned());
        Ok(())
    }

    fn max_address(&self) -> Result<u64, MemViewError> {
        Ok(self.data.len() as u64)
    }

    fn can_read_while_running(&self) -> bool {
        true
    }

    fn can_write_while_running(&self) -> bool {
        true
    }
}
