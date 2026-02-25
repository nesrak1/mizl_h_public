use super::constructor::{Constructor, ConstructorPrintElement, ContextOpTpl};
use super::expression::Expression;
use super::memory::{read_ctx_u32_bits_at, read_mem_u32_bits_at, read_mem_u64_bits_at, write_ctx_u32_bits_at};
use super::sla_file::{Sleigh, Symbol, SymbolInner};
use super::sym_subtable::SubtableSym;
use super::sym_value::ValueSym;
use super::sym_valuemap::ValuemapSym;
use super::sym_varlist::VarlistSym;
use crate::consts::arch::Endianness;
use crate::ffi::core_framework::prelude::*;
use crate::memory::memview::{MemView, MemViewError};
use crate::shared::fast_util::i64_to_str_fast;
use mizl_pm::FfiSerialize;
use smallvec::SmallVec;

pub enum DisasmProtoPart<'a> {
    Literal(&'a str),
    SymbolInfo(DisasmProtoSubsym<'a>),
    ExpressionInfo(DisasmProtoExpression<'a>),
}

pub enum DisasmInstructionPart {
    Literal(String),
    Operand(i32),
}

pub struct Disasm {
    pub sleigh: Sleigh,
    pub initial_ctx: Vec<u32>,
}

struct DisasmStackItem<'a> {
    pub ctor: &'a Constructor,
    pub print_elem_idx: usize,
    pub last_operand_idx: i32,
    pub op_offsets: Vec<u32>,
    pub read_position: u64,
    pub subsym_id: u32,
    pub ctor_idx: u32,
}

pub struct DisasmOperandStackItem {
    pub read_position: u64,
    pub subsym_id: u32,
    pub ctor_idx: u32,
    pub operand_ids: Vec<u32>,
}

pub struct DisasmPrototype<'a> {
    pub parts: SmallVec<DisasmProtoPart<'a>, 16>,
    pub length: u64,
}

pub struct DisasmProtoExpression<'a> {
    pub saved_ctx: Vec<u32>,
    pub saved_stack: DisasmOperandStackItem,
    pub expression: &'a Expression,
    pub offset: u64,
}

pub struct DisasmProtoSubsym<'a> {
    pub saved_ctx: Vec<u32>,
    pub saved_stack: DisasmOperandStackItem,
    pub symbol: &'a Symbol,
    pub offset: u64,
}

pub struct DisasmState<'a> {
    mem: &'a dyn MemView,
    ctx: Vec<u32>,
    start_addr: u64,
    end_addr: u64,
    _next2_addr: u64,
}

#[derive(FromPrimitive, ToPrimitive, Copy, Clone)]
pub enum DisasmDispInstructionRunType {
    Normal = 0,
    Mnemonic = 1,
    Register = 2,
    Number = 3,
}

#[derive(FfiSerialize)]
pub struct DisasmDispInstructionRun {
    pub length: u32,
    #[ffi_serialize_enum]
    pub run_type: DisasmDispInstructionRunType,
}

#[derive(FfiSerialize)]
pub struct DisasmDispInstruction {
    pub addr: u64,
    pub len: u64,
    pub text: String,
    pub runs: Vec<DisasmDispInstructionRun>,
}

impl DisasmDispInstructionRun {
    pub fn new(length: u32, run_type: DisasmDispInstructionRunType) -> DisasmDispInstructionRun {
        DisasmDispInstructionRun { length, run_type }
    }
}

impl DisasmState<'_> {
    pub fn new(mem: &dyn MemView, ctx: Vec<u32>, start_addr: u64) -> DisasmState {
        DisasmState {
            mem,
            ctx,
            start_addr,
            end_addr: start_addr,
            _next2_addr: start_addr,
        }
    }

    pub fn read_ctx_at(&self, off: u64, size: usize) -> Vec<u32> {
        self.ctx[off as usize..off as usize + size].to_vec()
    }

    pub fn read_mem_u32_at(&self, off: u64, big_endian: bool) -> Result<u32, MemViewError> {
        let mut addr = off;
        let endian = if big_endian {
            Endianness::BigEndian
        } else {
            Endianness::LittleEndian
        };
        self.mem.read_u32(&mut addr, endian)
    }

    pub fn read_mem_u32_bits_at(
        &self,
        off: u64,
        bit_off: i32,
        bit_size: i32,
        big_endian: bool,
    ) -> Result<u32, MemViewError> {
        read_mem_u32_bits_at(self.mem, off, bit_off, bit_size, big_endian)
    }

    pub fn read_ctx_u32_at(&self, off: u64) -> u32 {
        self.read_ctx_u32_bits_at((off * 8) as i32, 32)
    }

    pub fn read_mem_u64_bits_at(
        &self,
        off: u64,
        bit_off: i32,
        bit_size: i32,
        big_endian: bool,
    ) -> Result<u64, MemViewError> {
        read_mem_u64_bits_at(self.mem, off, bit_off, bit_size, big_endian)
    }

    pub fn read_ctx_u32_bits_at(&self, bit_off: i32, bit_size: i32) -> u32 {
        read_ctx_u32_bits_at(&self.ctx, bit_off, bit_size)
    }

    pub fn write_ctx_u32_bits_at(&mut self, bit_off: i32, bit_size: i32, value: u32) {
        write_ctx_u32_bits_at(&mut self.ctx, bit_off, bit_size, value);
    }

    pub fn get_context(&self) -> &Vec<u32> {
        &self.ctx
    }

    pub fn get_start_ins(&self) -> i64 {
        self.start_addr as i64
    }

    pub fn get_end_ins(&self) -> i64 {
        self.end_addr as i64
    }

    pub fn get_next2_ins(&self) -> i64 {
        panic!("inst_next2 supported yet");
        // self.next2_addr as i64
    }

    pub fn set_end_ins(&mut self, value: u64) {
        self.end_addr = value;
    }
}

impl DisasmPrototype<'_> {
    fn new(parts: SmallVec<DisasmProtoPart, 16>, length: u64) -> DisasmPrototype {
        DisasmPrototype { parts, length }
    }
}

impl DisasmProtoExpression<'_> {
    fn new(
        saved_ctx: Vec<u32>,
        saved_stack: DisasmOperandStackItem,
        expression: &Expression,
        offset: u64,
    ) -> DisasmProtoExpression {
        DisasmProtoExpression {
            saved_ctx,
            saved_stack,
            expression,
            offset,
        }
    }
}

impl DisasmProtoSubsym<'_> {
    fn new(
        saved_ctx: Vec<u32>,
        saved_stack: DisasmOperandStackItem,
        symbol: &Symbol,
        offset: u64,
    ) -> DisasmProtoSubsym {
        DisasmProtoSubsym {
            saved_ctx,
            saved_stack,
            symbol,
            offset,
        }
    }
}

impl DisasmOperandStackItem {
    fn from_stack_item(stack_item: &DisasmStackItem) -> DisasmOperandStackItem {
        DisasmOperandStackItem {
            read_position: stack_item.read_position,
            subsym_id: stack_item.subsym_id,
            ctor_idx: stack_item.ctor_idx,
            operand_ids: stack_item.ctor.operand_ids.clone(),
        }
    }
}

impl Disasm {
    pub fn new(sleigh: Sleigh, initial_ctx: Vec<u32>) -> Disasm {
        Disasm { sleigh, initial_ctx }
    }

    // hot path
    fn resolve_ctor(&self, state: &mut DisasmState, subtable_sym: &SubtableSym, at: u64) -> Result<i32, &str> {
        let mut decision = &subtable_sym.decision;
        let mut word_stack: SmallVec<u32, 3> = SmallVec::with_capacity(3);
        let mut word_stack_len = 1;

        // cache 32-bit words so we don't read multiple times for small bit segments
        word_stack.push(match state.read_mem_u32_at(at, true) {
            Ok(v) => v,
            Err(_) => return Err("<invalid read>"),
        });

        loop {
            if decision.size != 0 {
                let check_bits: u32;
                let decision_start = decision.start;
                let decision_size = decision.size;
                if decision.context {
                    check_bits = state.read_ctx_u32_bits_at(decision_start, decision_size);
                } else {
                    let word_stack_idx = decision_start / 32;
                    let word_stack_plus = ((decision_start + 32) & (!0x1f)) - (decision_start + decision_size);
                    let word_stack_end_idx = if word_stack_plus < 0 {
                        word_stack_idx + (-word_stack_plus) / 32
                    } else {
                        word_stack_idx
                    };
                    while word_stack_end_idx >= word_stack_len {
                        word_stack.push(match state.read_mem_u32_at((word_stack_len / 4) as u64, true) {
                            Ok(v) => v,
                            Err(_) => return Err("<invalid read>"),
                        });
                        word_stack_len += 1;
                    }

                    let unused_bits = 32 - decision_size;
                    // safety: we just added enough items to word_stack above
                    let mut tmp = unsafe { *word_stack.get_unchecked(word_stack_idx as usize) };
                    tmp <<= decision_start - (word_stack_idx * 32);
                    tmp >>= unused_bits;
                    if word_stack_plus < 0 {
                        // safety: ditto
                        let tmp2 = unsafe { *word_stack.get_unchecked((word_stack_idx + 1) as usize) } >> unused_bits;
                        tmp |= tmp2;
                        if word_stack_plus < -32 {
                            todo!("oops, tried to read more than two words");
                        }
                    }
                    check_bits = tmp;
                }
                // safety: assertion exists in Decision constructor to guarantee this works
                decision = unsafe { decision.children.get_unchecked(check_bits as usize) };
            } else {
                break;
            }
        }

        // find constructor
        for pair in &decision.pairs {
            let pattern = &pair.pattern;
            if pattern.is_match(state, at) {
                return Ok(pair.ctor_id);
            }
        }
        return Err("<pattern not found>");
    }

    fn get_value_sym_string(
        &self,
        state: &mut DisasmState,
        top_stack: &DisasmOperandStackItem,
        at: u64,
        sym: &Box<ValueSym>,
    ) -> String {
        let value = sym.patexp.evaluate(self, state, top_stack, at);
        i64_to_str_fast(value)
    }

    fn get_exp_string(
        &self,
        state: &mut DisasmState,
        top_stack: &DisasmOperandStackItem,
        at: u64,
        exp: &Expression,
    ) -> String {
        let value = exp.evaluate(self, state, top_stack, at);
        i64_to_str_fast(value)
    }

    fn get_varlist_sym_string(
        &self,
        state: &mut DisasmState,
        top_stack: &DisasmOperandStackItem,
        at: u64,
        sym: &Box<VarlistSym>,
    ) -> Result<&str, ()> {
        let value = sym.patexp.evaluate(self, state, top_stack, at);
        let var_idx = sym.var_ids[value as usize];
        if var_idx == u32::MAX {
            return Err(());
        }

        let varnode_sym_box = &self.sleigh.symbol_table.symbols[var_idx as usize];
        Ok(&varnode_sym_box.name)
    }

    fn get_valuemap_sym_string(
        &self,
        state: &mut DisasmState,
        top_stack: &DisasmOperandStackItem,
        at: u64,
        sym: &Box<ValuemapSym>,
    ) -> String {
        let value = sym.patexp.evaluate(self, state, top_stack, at);
        let var_value = sym.values[value as usize];
        i64_to_str_fast(var_value)
    }

    fn set_context(
        &self,
        state: &mut DisasmState,
        context_ops: &Vec<ContextOpTpl>,
        top_stack: &DisasmOperandStackItem,
        at: u64,
    ) {
        for context_op in context_ops {
            let exp_value = context_op.expression.evaluate(self, state, top_stack, at) as u32;

            let old_ctx_val = state.read_ctx_u32_at((context_op.word_start * 32) as u64);
            let new_ctx_val = (old_ctx_val & (!context_op.mask)) | (exp_value << context_op.bit_shift);
            state.write_ctx_u32_bits_at(context_op.word_start * 32, 32, new_ctx_val);
        }
    }

    // todo: error type
    pub fn disasm_proto(&self, mem: &dyn MemView, at: u64) -> Result<DisasmPrototype, ()> {
        let mut state = DisasmState::new(mem, self.initial_ctx.clone(), at);

        let root_scope = &self.sleigh.symbol_table.scopes[0];
        let instruction_subtable_idx = match root_scope.lookup.get("instruction") {
            Some(v) => *v,
            None => panic!("expected instruction in root scope"),
        };

        let sleigh_symbols = &self.sleigh.symbol_table.symbols;

        let subtable_sym_box = &sleigh_symbols[instruction_subtable_idx];
        let subtable_sym = if let SymbolInner::SubtableSym(v) = &subtable_sym_box.inner {
            v
        } else {
            panic!("not a subtable symbol")
        };

        let mut stack: SmallVec<DisasmStackItem, 16> = SmallVec::new();
        let mut proto_parts: SmallVec<DisasmProtoPart, 16> = SmallVec::new();

        let base_ctor_idx = match self.resolve_ctor(&mut state, subtable_sym, at) {
            Ok(c) => c,
            Err(_) => return Err(()),
        };
        let base_ctor = &subtable_sym.ctors[base_ctor_idx as usize];

        // avoids recursion
        stack.push(DisasmStackItem {
            ctor: base_ctor,
            print_elem_idx: 0,
            last_operand_idx: -1,
            op_offsets: vec![u32::MAX; base_ctor.operand_ids.len()],
            read_position: at,
            subsym_id: subtable_sym_box.id,
            ctor_idx: base_ctor_idx as u32,
        });
        let first_op_top_stack = DisasmOperandStackItem::from_stack_item(stack.last().unwrap());
        self.set_context(&mut state, &base_ctor.context_ops, &first_op_top_stack, at);

        let mut end_pos = at + base_ctor.min_length as u64;
        while !stack.is_empty() {
            let mut elem_to_add: Option<DisasmStackItem> = None;

            let top_stack = stack.last().expect("stack is empty");
            if top_stack.print_elem_idx >= top_stack.ctor.print_elements.len() {
                stack.pop();
                // no reason to edit op_offsets if there's no more stack
                if !stack.is_empty() {
                    let prev_top_stack = stack.last_mut().expect("stack is empty");
                    if prev_top_stack.last_operand_idx != -1 {
                        // todo: store end pos into stack item
                        // end_pos may not be trustworthy since
                        // operands could (theoretically) appear
                        // out of order in memory space
                        prev_top_stack.op_offsets[prev_top_stack.last_operand_idx as usize] = (end_pos - at) as u32;
                    }
                }
                continue;
            }

            let mut last_oper_idx = -1;
            let print_elem = &top_stack.ctor.print_elements[top_stack.print_elem_idx];
            match print_elem {
                ConstructorPrintElement::Literal(s) => {
                    proto_parts.push(DisasmProtoPart::Literal(s));
                }
                ConstructorPrintElement::Operand(oper_idx) => {
                    last_oper_idx = *oper_idx;
                    let symbol_idx = top_stack.ctor.operand_ids[*oper_idx as usize];
                    let operand_sym_box = &sleigh_symbols[symbol_idx as usize];
                    let operand_sym = if let SymbolInner::OperandSym(v) = &operand_sym_box.inner {
                        v
                    } else {
                        panic!("not an operand symbol")
                    };

                    let operand_off = if operand_sym.offset_base == -1 {
                        top_stack.read_position + operand_sym.rel_offset as u64
                    } else {
                        // hopefully this is filled in already...
                        at + (top_stack.op_offsets[operand_sym.offset_base as usize] as u64)
                    };

                    // if this is further than we've been before, move end_pos to this position
                    let operand_end_pos = operand_off + operand_sym.min_length as u64;
                    if operand_end_pos > end_pos {
                        end_pos = operand_end_pos;
                    }

                    let subsym_idx = operand_sym.subsym;
                    if subsym_idx != u32::MAX {
                        // dynamic value
                        let operand_subsym_box = &sleigh_symbols[subsym_idx as usize];
                        match &operand_subsym_box.inner {
                            SymbolInner::ValueSym(_)
                            | SymbolInner::VarlistSym(_)
                            | SymbolInner::ValuemapSym(_)
                            | SymbolInner::VarnodeSym(_) => {
                                let op_top_stack = DisasmOperandStackItem::from_stack_item(top_stack);
                                let saved_ctx = state.get_context().clone();
                                let exp_info =
                                    DisasmProtoSubsym::new(saved_ctx, op_top_stack, operand_subsym_box, operand_off);
                                proto_parts.push(DisasmProtoPart::SymbolInfo(exp_info));
                            }
                            SymbolInner::SubtableSym(subtable_sym) => {
                                let sub_ctor_idx = match self.resolve_ctor(&mut state, subtable_sym, operand_off) {
                                    Ok(c) => c,
                                    Err(_) => return Err(()),
                                };

                                let sub_ctor = &subtable_sym.ctors[sub_ctor_idx as usize];
                                let sub_ctor_stack_item = DisasmStackItem {
                                    ctor: sub_ctor,
                                    print_elem_idx: 0,
                                    last_operand_idx: -1,
                                    op_offsets: vec![u32::MAX; sub_ctor.operand_ids.len()],
                                    read_position: operand_off,
                                    subsym_id: operand_subsym_box.id,
                                    ctor_idx: sub_ctor_idx as u32,
                                };

                                if sub_ctor.context_ops.len() > 0 {
                                    let elem_to_add_stack =
                                        DisasmOperandStackItem::from_stack_item(&sub_ctor_stack_item);
                                    //let op_top_stack = DisasmOperandStackItem::from_stack_item(top_stack);
                                    self.set_context(
                                        &mut state,
                                        &sub_ctor.context_ops,
                                        &elem_to_add_stack,
                                        operand_off,
                                    );
                                }

                                elem_to_add = Some(sub_ctor_stack_item);

                                // if this is further than we've been before, move end_pos to this position
                                let ctor_end_pos = operand_off + sub_ctor.min_length as u64;
                                if ctor_end_pos > end_pos {
                                    end_pos = ctor_end_pos;
                                }
                            }
                            _ => panic!("unsupported symbol type for operand"),
                        };
                    } else if let Some(def_exp) = &operand_sym.def_exp {
                        // static value
                        let op_top_stack = DisasmOperandStackItem::from_stack_item(top_stack);
                        let saved_ctx = state.get_context().clone();
                        let exp_info = DisasmProtoExpression::new(saved_ctx, op_top_stack, def_exp, operand_off);
                        proto_parts.push(DisasmProtoPart::ExpressionInfo(exp_info));
                    } else {
                        panic!("either operand subsymbol or defexp should've been set");
                    }

                    // ghidra code suggests we should be pushing the _start_ of the
                    // operand, but it doesn't really make any sense unless we push
                    // the _end_ of the operand
                    let top_stack_mut = stack.last_mut().expect("stack is empty");
                    top_stack_mut.op_offsets[*oper_idx as usize] = (operand_end_pos - at) as u32;
                }
            }

            let top_stack_mut = stack.last_mut().expect("stack is empty");
            top_stack_mut.print_elem_idx += 1;
            if last_oper_idx != -1 {
                top_stack_mut.last_operand_idx = last_oper_idx;
            }

            if let Some(item) = elem_to_add {
                stack.push(item);
            }
        }

        let length = end_pos - at;
        let prototype = DisasmPrototype::new(proto_parts, length);
        return Ok(prototype);
    }

    fn get_proto_display(
        &self,
        mem: &dyn MemView,
        at: u64,
        end_pos: u64,
        prototype: &DisasmPrototype,
    ) -> Result<(String, Vec<DisasmDispInstructionRun>), ()> {
        let mut final_str = String::with_capacity(64);
        let mut runs: Vec<DisasmDispInstructionRun> = Vec::new();
        let mut is_mnemonic = true;

        fn add_run(
            add_str: &str,
            run_type: DisasmDispInstructionRunType,
            runs: &mut Vec<DisasmDispInstructionRun>,
            final_str: &mut String,
        ) {
            *final_str += add_str;
            runs.push(DisasmDispInstructionRun::new(add_str.len() as u32, run_type));
        }

        // single base state to avoid unnecessary allocations
        let ctx_size = self.initial_ctx.len();
        let mut state: DisasmState = DisasmState::new(mem, vec![0u32; ctx_size], at);
        state.set_end_ins(end_pos);

        for elem in &prototype.parts {
            match elem {
                DisasmProtoPart::Literal(v) => {
                    // yes, the only thing marking the end of
                    // a mnemonic is a space character
                    // todo: is this guaranteed to be alone?
                    if v.contains(" ") {
                        is_mnemonic = false;
                    }
                    if is_mnemonic {
                        add_run(&v, DisasmDispInstructionRunType::Mnemonic, &mut runs, &mut final_str);
                    } else {
                        add_run(&v, DisasmDispInstructionRunType::Normal, &mut runs, &mut final_str);
                    }
                }
                DisasmProtoPart::ExpressionInfo(info) => {
                    state.ctx.clear();
                    state.ctx.extend_from_slice(&info.saved_ctx);

                    let v = self.get_exp_string(&mut state, &info.saved_stack, info.offset, info.expression);
                    add_run(&v, DisasmDispInstructionRunType::Number, &mut runs, &mut final_str);
                }
                DisasmProtoPart::SymbolInfo(info) => {
                    state.ctx.clear();
                    state.ctx.extend_from_slice(&info.saved_ctx);

                    let op_top_stack = &info.saved_stack;
                    let operand_off = info.offset;

                    let inner = &info.symbol.inner;
                    let v = match inner {
                        SymbolInner::ValueSym(value_sym) => {
                            &self.get_value_sym_string(&mut state, &op_top_stack, operand_off, value_sym)
                        }
                        SymbolInner::VarlistSym(varlist_sym) => {
                            self.get_varlist_sym_string(&mut state, &op_top_stack, operand_off, varlist_sym)?
                        }
                        SymbolInner::ValuemapSym(valuemap_sym) => {
                            &self.get_valuemap_sym_string(&mut state, &op_top_stack, operand_off, valuemap_sym)
                        }
                        SymbolInner::VarnodeSym(_) => &info.symbol.name,
                        _ => panic!("unsupported symbol type for operand"),
                    };

                    match inner {
                        SymbolInner::ValueSym(_) => {
                            add_run(&v, DisasmDispInstructionRunType::Number, &mut runs, &mut final_str);
                        }
                        SymbolInner::ValuemapSym(_) => {
                            add_run(&v, DisasmDispInstructionRunType::Number, &mut runs, &mut final_str);
                        }
                        SymbolInner::VarlistSym(_) => {
                            add_run(&v, DisasmDispInstructionRunType::Register, &mut runs, &mut final_str);
                        }
                        SymbolInner::VarnodeSym(_) => {
                            add_run(&v, DisasmDispInstructionRunType::Register, &mut runs, &mut final_str);
                        }
                        _ => panic!("unsupported symbol type for operand"),
                    }
                }
            };
        }

        Ok((final_str, runs))
    }

    pub fn disasm_display(&self, mem: &dyn MemView, at: u64) -> Result<DisasmDispInstruction, ()> {
        let at_val = at;
        let prototype = self.disasm_proto(mem, at_val)?;
        let (text, runs) = self.get_proto_display(mem, at_val, at_val + prototype.length, &prototype)?;

        let display_ins = DisasmDispInstruction {
            addr: at_val,
            len: prototype.length,
            text,
            runs,
        };
        Ok(display_ins)
    }
}
