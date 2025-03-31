use crate::consts::arch::Endianness;
use std::fmt;

#[derive(Debug)]
pub enum MemViewError {
    EndOfStream,
    ReadAccessDenied,
    WriteAccessDenied,
    NotLoaded,
    Unsupported(&'static str),
}

impl fmt::Display for MemViewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MemViewError::EndOfStream => write!(f, "address was further than stream length"),
            MemViewError::ReadAccessDenied => write!(f, "data was unable to be read"),
            MemViewError::WriteAccessDenied => write!(f, "data was unable to be written"),
            MemViewError::NotLoaded => write!(f, "memory is not yet loaded or was recently unloaded"),
            MemViewError::Unsupported(s) => write!(f, "the operation is not supported: {}", s),
        }
    }
}

pub trait MemView {
    fn read_bytes(&self, addr: &mut u64, out_data: &mut [u8], count: i32) -> Result<(), MemViewError>;
    fn write_bytes(&mut self, addr: &mut u64, value: &[u8]) -> Result<(), MemViewError>;
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

pub struct StaticMemoryView {
    data: Vec<u8>,
}

impl StaticMemoryView {
    pub fn new(data: Vec<u8>) -> StaticMemoryView {
        StaticMemoryView { data }
    }
}

impl MemView for StaticMemoryView {
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

    fn can_read_while_running(&self) -> bool {
        true
    }

    fn can_write_while_running(&self) -> bool {
        true
    }
}
