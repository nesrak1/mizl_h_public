use crate::{
    consts::arch::Endianness,
    database::{gbf::GbfFile, gbf_node_kind::GbfNodeKind},
    memory::memview::{MemView, MemViewError},
};

// todo: currently unused/unchecked. NEEDS TESTING!
// a memview that reads a specific ChainedBuffer
pub struct GbfChainedBufMemView<'a> {
    gbf: &'a GbfFile,
    buffer_size: i32,
    _obfuscated: bool, // for antivirus detection, currently unsupported
    index_map: Vec<i32>,
    buffer_map: Vec<i32>,
}

impl<'a> GbfChainedBufMemView<'a> {
    pub fn new(gbf: &'a GbfFile, mv: &Box<dyn MemView>, nid: i32) -> Result<GbfChainedBufMemView<'a>, MemViewError> {
        let endian = Endianness::BigEndian; // always big endian
        let at = &mut gbf.get_buffer_address(nid);

        let node_kind = mv.read_u8(at)?;

        let obf_buffer_size = mv.read_u32(at, endian)?;
        // buffer_size seems to be the whole combined buffer size, not this one buffer
        let buffer_size = (obf_buffer_size & 0x7fffffff) as i32;
        let obfuscated = (obf_buffer_size & 0x80000000) != 0;

        if obfuscated {
            panic!("obfuscated chained buffer not supported yet");
        }

        if node_kind == GbfNodeKind::CHAINED_BUFFER_DATA {
            let index_map: Vec<i32> = Vec::with_capacity(0);
            let buffer_map: Vec<i32> = vec![nid];
            return Ok(GbfChainedBufMemView {
                gbf,
                buffer_size,
                _obfuscated: obfuscated,
                index_map,
                buffer_map,
            });
        } else if node_kind == GbfNodeKind::CHAINED_BUFFER_INDEX {
            let gbf_buffer_size = gbf.get_buffer_size();

            let chain_data_len = gbf_buffer_size - Self::get_chain_data_prefix_len(true);
            let chain_index_len = gbf_buffer_size - 1 - 4 - 4;
            let index_count = (((buffer_size as u64) - 1) / chain_data_len) + 1;
            let indexes_per_buffer = chain_index_len / 4;
            let at_chain = &mut *at;

            let mut index_map: Vec<i32> = Vec::with_capacity(index_count as usize);
            // todo: precompute max size and use with_capacity
            let mut buffer_map: Vec<i32> = Vec::new();

            index_map.push(nid);

            let last_index = std::cmp::max(index_count - 1, 0);
            for i in 0..index_count {
                let next_buffer_index = mv.read_i32(at_chain, endian)?;
                for _ in 0..indexes_per_buffer {
                    buffer_map.push(mv.read_i32(at_chain, endian)?);
                }

                if i != last_index {
                    index_map.push(next_buffer_index); // currently unused since we are readonly
                    *at_chain = gbf.get_buffer_address(next_buffer_index);
                    *at_chain += 1 + 4 + 4; // skip other fields
                }
            }

            return Ok(GbfChainedBufMemView {
                gbf,
                buffer_size,
                _obfuscated: obfuscated,
                index_map,
                buffer_map,
            });
        } else {
            let err_str = format!("unexpected block id {} while reading chained buffer", node_kind);
            return Err(MemViewError::generic_dynamic(err_str));
        }
    }

    fn read_bytes_from_buffer(
        &self,
        buffer_index: i32,
        buffer_offset: usize,
        out_data: &mut [u8],
        out_offset: usize,
        len: i32,
    ) -> Result<i32, MemViewError> {
        let chain_data_len = self.get_chain_data_len(); // make this an arg maybe?

        // todo: assuming buffer_offset < chain_data_len
        let remaining_space = (chain_data_len as usize) - buffer_offset;
        let read_len = std::cmp::min(remaining_space, len as usize);
        // todo: assuming buffer_index is in bounds of buffer_map
        let buffer_id = self.buffer_map[buffer_index as usize];
        if buffer_id < 0 {
            // buffer is not initialized yet, fill with zeros
            out_data[out_offset..(out_offset + read_len)].fill(0);
        } else {
            let mut read_addr = self.gbf.get_buffer_address(buffer_id);
            read_addr += Self::get_chain_data_prefix_len(self.is_indexed());

            self.gbf.mv.read_bytes(
                &mut read_addr,
                &mut out_data[out_offset..(out_offset + read_len)],
                read_len as i32,
            )?;
        }

        Ok(read_len as i32)
    }

    fn is_indexed(&self) -> bool {
        self.index_map.len() > 0
    }

    fn get_chain_data_len(&self) -> u64 {
        let gbf_buffer_size = self.gbf.get_buffer_size();
        gbf_buffer_size - Self::get_chain_data_prefix_len(self.is_indexed())
    }

    fn get_chain_data_prefix_len(is_indexed: bool) -> u64 {
        if is_indexed {
            1 // indexed chain data has no obf_buffer_size
        } else {
            1 + 4
        }
    }
}

impl<'a> MemView for GbfChainedBufMemView<'a> {
    fn read_bytes(&self, addr: &mut u64, out_data: &mut [u8], count: i32) -> Result<(), MemViewError> {
        if (*addr + (count as u64) - 1) >= self.buffer_size as u64 {
            return Err(MemViewError::EndOfStream);
        } else if count < 0 {
            return Ok(());
        }

        let chain_data_len = self.get_chain_data_len();

        let mut out_data_offset = 0usize;
        let mut index = (*addr / chain_data_len) as i32;
        let mut buffer_data_offset = (*addr % chain_data_len) as usize;
        let mut len = count;
        while len > 0 {
            let n = self.read_bytes_from_buffer(index, buffer_data_offset, out_data, out_data_offset, len)?;
            index += 1;
            *addr += n as u64;
            out_data_offset += n as usize;
            len -= n;
            buffer_data_offset = 0;
        }

        Ok(())
    }

    fn write_bytes(&mut self, _addr: &mut u64, _value: &[u8]) -> Result<(), MemViewError> {
        panic!("writing to chained buffer not supported yet");
    }

    fn max_address(&self) -> Result<u64, MemViewError> {
        Ok(self.buffer_size as u64)
    }

    fn can_read_while_running(&self) -> bool {
        true
    }

    fn can_write_while_running(&self) -> bool {
        true
    }
}
