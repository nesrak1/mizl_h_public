use crate::memory::memview::{MemView, MemViewError};
use std::collections::HashMap;

// generic memory stored in chunks. this allows for storing memory at very
// different addresses without storing all of the memory in-between them.

pub struct FreeMemChunk {
    range_start: usize,
    range_len: usize,
    data: Vec<u8>,
}

pub struct ChunkedFreeMemView {
    chunks: HashMap<u64, FreeMemChunk>,
    chunk_len: usize,
}

impl FreeMemChunk {
    fn new(range_start: usize, range_len: usize, len: usize) -> FreeMemChunk {
        FreeMemChunk {
            range_start,
            range_len,
            data: vec![0; len],
        }
    }
}

impl ChunkedFreeMemView {
    pub fn new(chunk_len: usize) -> ChunkedFreeMemView {
        ChunkedFreeMemView {
            chunks: HashMap::new(),
            chunk_len,
        }
    }
}

impl MemView for ChunkedFreeMemView {
    fn read_bytes(&self, addr: &mut u64, out_data: &mut [u8], mut count: i32) -> Result<(), MemViewError> {
        if count < 0 {
            return Ok(());
        }

        let chunk_len = self.chunk_len as u64;
        let start_addr = *addr;
        let end_addr = start_addr + count as u64;

        let start_chunk_idx = start_addr / chunk_len;
        let mut cur_chunk_idx = start_chunk_idx;
        let mut cur_src_chunk_off = start_addr % chunk_len;
        let mut cur_dst_addr = 0usize;

        loop {
            let cur_src_addr = cur_chunk_idx * chunk_len + cur_src_chunk_off;
            if cur_src_addr >= end_addr {
                break;
            }

            let chunk_info = match self.chunks.get(&cur_chunk_idx) {
                Some(v) => v,
                None => return Err(MemViewError::EndOfStream),
            };

            let chunk_range_end = chunk_info.range_start + chunk_info.range_len;

            let chunk_start_idx = cur_src_chunk_off as usize;
            let chunk_bytes_left = chunk_len - chunk_start_idx as u64;

            // take bytes to the end of chunk or as many bytes as we want left, whichever is less
            let bytes_to_read = chunk_bytes_left.min(count as u64) as usize;
            let chunk_end_idx = chunk_start_idx + bytes_to_read;

            // we want to read bytes out of range, not good
            if chunk_start_idx < chunk_info.range_start || chunk_end_idx > chunk_range_end {
                return Err(MemViewError::EndOfStream);
            }

            let src_chunk_data = &chunk_info.data[chunk_start_idx..chunk_end_idx];
            out_data[cur_dst_addr..cur_dst_addr + bytes_to_read].copy_from_slice(src_chunk_data);

            // reset offset so we always read from
            // the beginning after the first chunk
            cur_src_chunk_off = 0;
            cur_chunk_idx += 1;

            cur_dst_addr += bytes_to_read;
            count -= bytes_to_read as i32;
        }

        *addr = end_addr;
        Ok(())
    }

    fn write_bytes(&mut self, addr: &mut u64, value: &[u8]) -> Result<(), MemViewError> {
        if value.len() > i32::MAX as usize {
            return Err(MemViewError::Unsupported("cannot use write more than i32::MAX bytes"));
        }

        let mut count = value.len() as u64;
        let chunk_len = self.chunk_len as u64;
        let start_addr = *addr;
        let end_addr = start_addr + count as u64;

        let start_chunk_idx = start_addr / chunk_len;
        let mut cur_chunk_idx = start_chunk_idx;
        let mut cur_dst_chunk_off = start_addr % chunk_len;
        let mut cur_src_addr = 0usize;

        loop {
            let cur_dst_addr = cur_chunk_idx * chunk_len + cur_dst_chunk_off;
            if cur_dst_addr >= end_addr {
                break;
            }

            let chunk_start_idx = cur_dst_chunk_off as usize;
            let chunk_bytes_left = chunk_len - chunk_start_idx as u64;

            // write bytes to the end of chunk or as many bytes as we have left, whichever is less
            let bytes_to_write = chunk_bytes_left.min(count) as usize;
            let chunk_end_idx = chunk_start_idx + bytes_to_write;

            let chunk_info = self.chunks.entry(cur_chunk_idx).or_insert_with(|| {
                FreeMemChunk::new(chunk_start_idx, chunk_end_idx - chunk_start_idx, chunk_len as usize)
            });

            if chunk_start_idx < chunk_info.range_start {
                let difference = chunk_info.range_start - chunk_start_idx;
                chunk_info.range_start = chunk_start_idx;
                chunk_info.range_len += difference;
            }

            let chunk_range_end = chunk_info.range_start + chunk_info.range_len;
            if chunk_end_idx > chunk_range_end {
                let difference = chunk_end_idx - chunk_range_end;
                chunk_info.range_len += difference;
            }

            let src_chunk_data = &value[cur_src_addr..cur_src_addr + bytes_to_write];
            chunk_info.data[chunk_start_idx..chunk_start_idx + bytes_to_write].copy_from_slice(src_chunk_data);

            // reset offset so we always read from
            // the beginning after the first chunk
            cur_dst_chunk_off = 0;
            cur_chunk_idx += 1;

            cur_src_addr += bytes_to_write;
            count -= bytes_to_write as u64;
        }

        *addr = end_addr;
        Ok(())
    }

    fn can_read_while_running(&self) -> bool {
        true
    }

    // unsure yet if this is a good idea
    fn can_write_while_running(&self) -> bool {
        false
    }
}
