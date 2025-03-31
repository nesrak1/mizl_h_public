use std::collections::HashMap;

use crate::memory::memview::{MemView, MemViewError};

pub enum BreakpointKind {
    Normal,
}

pub struct BreakpointEntry {
    addr: u64,
    _enabled: bool,
    _hit: bool,
    _bp_kind: BreakpointKind,
    bp_bytes: Vec<u8>,
    orig_bytes: Vec<u8>,
}

pub struct BreakpointContainer {
    // sorted breakpoints for binary searching
    bps_sorted: Vec<BreakpointEntry>,
    // lookup for bps_sorted by breakpoint id
    bps_by_id: HashMap<u32, usize>,
    // current breakpoint id
    bp_id: u32,
}

// wrapper to replace memory from a memview with
// the original memory under a breakpoint.
pub struct BreakpointWrapMemView<'a, MV>
where
    MV: MemView,
{
    pub mem_view: &'a mut MV,
    pub bp_cont: &'a BreakpointContainer,
}

impl BreakpointEntry {
    pub fn new(addr: u64, bp_bytes: Vec<u8>, orig_bytes: Vec<u8>) -> BreakpointEntry {
        BreakpointEntry {
            addr,
            _enabled: true,
            _hit: false,
            _bp_kind: BreakpointKind::Normal,
            bp_bytes,
            orig_bytes,
        }
    }
}

impl BreakpointContainer {
    pub fn new() -> BreakpointContainer {
        BreakpointContainer {
            bps_sorted: Vec::new(),
            bps_by_id: HashMap::new(),
            bp_id: 0,
        }
    }

    pub fn add_breakpoint(&mut self, entry: BreakpointEntry) -> u32 {
        let addr = entry.addr;
        let insert_idx = match self.bps_sorted.binary_search_by(|e| e.addr.cmp(&addr)) {
            Ok(i) => i,
            Err(i) => i,
        };

        self.bps_sorted.insert(insert_idx, entry);
        self.bps_by_id.insert(self.bp_id, self.bps_sorted.len() - 1);
        self.bp_id += 1;

        self.bp_id - 1
    }

    // todo: opto this somehow
    // we do a lot of short reads so this will be a little bad...
    pub fn fixup_bp_memory(&self, data: &mut [u8], data_addr: u64) {
        let mem_len = data.len();
        let mem_start = data_addr;
        let mem_end = mem_start + data.len() as u64;
        let (bp_start_idx, bp_end_idx) = Self::find_bps_in_range(&self, mem_start, mem_end);
        for bp in &self.bps_sorted[bp_start_idx..bp_end_idx] {
            let bp_mem_len = bp.bp_bytes.len();
            let bp_mem_start: isize = bp.addr.wrapping_sub(data_addr) as usize as isize;
            let (src_start, dst_start) = if bp_mem_start < 0 {
                ((-bp_mem_start) as usize, 0)
            } else {
                (0, bp_mem_start as usize)
            };

            let count = (bp_mem_len - src_start).min(mem_len - dst_start);
            data[dst_start..dst_start + count].copy_from_slice(&bp.orig_bytes[src_start..src_start + count]);
        }
    }

    // todo: check this for correctness
    fn find_bps_in_range(&self, start: u64, end: u64) -> (usize, usize) {
        // todo: I don't really like supporting stacked breakpoints,
        // but if we do, this most likely doesn't find the leftmost
        // breakpoint at the same address. we should check for that.
        let start_idx = match self.bps_sorted.binary_search_by(|e| e.addr.cmp(&start)) {
            Ok(i) => i,
            Err(i) => i,
        };

        let end_idx =
            match self.bps_sorted[start_idx..].binary_search_by(|e| (e.addr + e.bp_bytes.len() as u64).cmp(&end)) {
                Ok(i) => i + 1,
                Err(i) => i,
            };

        if start_idx < end_idx {
            (start_idx, end_idx)
        } else {
            (0, 0)
        }
    }
}

impl<'a, MV> BreakpointWrapMemView<'a, MV>
where
    MV: MemView,
{
    pub fn new(mem_view: &'a mut MV, bp_cont: &'a BreakpointContainer) -> BreakpointWrapMemView<'a, MV> {
        BreakpointWrapMemView { mem_view, bp_cont }
    }
}

impl<MV> MemView for BreakpointWrapMemView<'_, MV>
where
    MV: MemView,
{
    fn read_bytes(&self, addr: &mut u64, out_data: &mut [u8], count: i32) -> Result<(), MemViewError> {
        let orig_addr = *addr;
        match self.mem_view.read_bytes(addr, out_data, count) {
            Ok(_) => (),
            Err(e) => return Err(e),
        };

        self.bp_cont.fixup_bp_memory(out_data, orig_addr);
        Ok(())
    }

    fn write_bytes(&mut self, addr: &mut u64, value: &[u8]) -> Result<(), MemViewError> {
        match self.mem_view.write_bytes(addr, value) {
            Ok(_) => (),
            Err(e) => return Err(e),
        };

        // todo: we currently don't handle if you write over a breakpoint :(
        Ok(())
    }

    fn can_read_while_running(&self) -> bool {
        self.mem_view.can_read_while_running()
    }

    fn can_write_while_running(&self) -> bool {
        self.mem_view.can_write_while_running()
    }
}
