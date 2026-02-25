// for 1-byte boundaries

use crate::{
    consts::arch::Endianness,
    memory::memview::{MemView, MemViewError},
};

// from ghidra source
fn flip_u32_byte_order(mut value: u32, mut byte_count: i32) -> u32 {
    let mut res = 0;
    while byte_count > 0 {
        res <<= 8;
        res |= value & 0xff;
        value >>= 8;
        byte_count -= 1;
    }
    res
}

fn flip_u64_byte_order(mut value: u64, mut byte_count: i32) -> u64 {
    let mut res = 0;
    while byte_count > 0 {
        res <<= 8;
        res |= value & 0xff;
        value >>= 8;
        byte_count -= 1;
    }
    res
}

pub fn read_mem_u32_bits_at(
    mem: &dyn MemView,
    mut off: u64,
    bit_off: i32,
    bit_size: i32,
    big_endian: bool,
) -> Result<u32, MemViewError> {
    let start_bit = bit_off & 0x7;
    off += (bit_off / 8) as u64;
    let byte_count = (start_bit + bit_size - 1) / 8 + 1;

    let mut addr = off;
    let mut res = mem.read_u32(&mut addr, Endianness::BigEndian)?;
    res <<= start_bit; // move starting bit to the highest position
    res >>= 32 - bit_size; // shift to the bottom of int
    if !big_endian {
        res = flip_u32_byte_order(res, byte_count);
    }
    Ok(res)
}

pub fn read_mem_u64_bits_at(
    mem: &dyn MemView,
    off: u64,
    bit_off: i32,
    bit_size: i32,
    big_endian: bool,
) -> Result<u64, MemViewError> {
    let start_bit = bit_off & 0x7;
    let byte_count = (start_bit + bit_size - 1) / 8 + 1;

    let mut addr = off;
    let mut res = mem.read_u64(&mut addr, Endianness::BigEndian)?;
    res <<= start_bit; // move starting bit to the highest position
    res >>= 64 - bit_size; // shift to the bottom of int
    if !big_endian {
        res = flip_u64_byte_order(res, byte_count);
    }
    Ok(res)
}

// for 4-byte boundaries
pub fn read_ctx_u32_bits_at(ctx: &[u32], bit_off: i32, bit_size: i32) -> u32 {
    let bit_offset = bit_off & 0x1f;
    let mut start_byte = (bit_off / 32) as u64;
    let word_count = (bit_offset + bit_size - 1) / 32 + 1;
    if word_count == 0 {
        return 0;
    } else if word_count > 2 {
        panic!("can't read more than two words from context at a time");
    }

    let mut res = ctx[start_byte as usize]; // get int containing bits
    let mut unused_bits = 32 - bit_size;
    res <<= bit_offset; // shift startbit to highest position
    res >>= unused_bits;
    let remaining = bit_size - 32 + bit_offset;
    if remaining > 0 && ((start_byte + 1) as usize) < ctx.len() {
        start_byte += 1;
        unused_bits = 32 - remaining;
        let res2 = ctx[start_byte as usize] >> unused_bits;
        res |= res2;
    }
    res
}

pub fn write_ctx_u32_bits_at(ctx: &mut [u32], bit_off: i32, bit_size: i32, value: u32) {
    let start_bit = bit_off & 0x1f;
    let start_byte = (bit_off / 32) as usize;
    if start_bit + bit_size > 32 {
        panic!("bit range can not exceed 32 bit boundary");
    }

    let mask = (((1u64 << bit_size) - 1) as u32) << start_bit;
    ctx[start_byte] = (ctx[start_byte] & (!mask)) | (mask & value);
}

// variant that writes to the context via contextfield bit ranges
// unfortunately for us, bit ranges work on the byte level instead
// of the context word level, so we have to do some trickery.
pub fn write_ctx_u32_bits_range(ctx: &mut [u32], bit_low: i32, bit_high: i32, value: u32) {
    if bit_high + 1 - bit_low > 32 {
        panic!("bit range can not exceed 32 bit boundary");
    }

    let mut start_bit = bit_low & 7;
    let mut bits_left = bit_high + 1 - bit_low;
    let mut cur_value = value;
    let start_byte = (bit_low / 32) as usize;
    let end_byte = (bit_high / 32) as usize;

    for byte_idx in start_byte..end_byte + 1 {
        let ctx_elm_idx = byte_idx / 8;
        let ctx_byt_idx = 3 - (byte_idx & 3);
        let ctx_shift = ctx_byt_idx * 8;

        let bit_count = bits_left.min(8 - start_bit);
        let bit_mask = ((1u64 << bit_count) - 1) as u32;
        let mask_shift = 8 - start_bit - bit_count;
        let ctx_masked = bit_mask << mask_shift;
        let value_masked = (cur_value & bit_mask) << mask_shift;
        ctx[ctx_elm_idx] = (ctx[ctx_elm_idx] & !(ctx_masked << ctx_shift)) | (value_masked << ctx_shift);

        bits_left -= bit_count;
        cur_value >>= bit_count;
        start_bit = 0;
    }
}
