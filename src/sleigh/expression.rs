use super::disasm::DisasmState;
use crate::sleigh::consts::{AttributeId, ElementId};
use crate::sleigh::disasm::{Disasm, DisasmOperandStackItem};
use crate::sleigh::sla_file::SymbolInner;
use crate::sleigh::sla_reader::{SlaBinReader, SlaElement};

pub struct TokenField {
    big_endian: bool,
    sign_bit: bool,
    bit_start: i32,
    bit_end: i32,
    byte_start: i32,
    byte_end: i32,
    shift: i32,
}

pub struct ContextField {
    sign_bit: bool,
    bit_start: i32,
    bit_end: i32,
    byte_start: i32,
    byte_end: i32,
    shift: i32,
}

pub struct OperandValue {
    index: i32,
    sym_id: u32,
    ctor_idx: u32,
}

pub enum Expression {
    TokenField(Box<TokenField>),
    ContextField(Box<ContextField>),
    ConstantValue(i64),
    OperandValue(Box<OperandValue>),
    StartInstructionValue,
    EndInstructionValue,
    Next2InstructionValue,
    AddExpression(Box<(Expression, Expression)>),
    SubExpression(Box<(Expression, Expression)>),
    MultExpression(Box<(Expression, Expression)>),
    DivExpression(Box<(Expression, Expression)>),
    LeftShiftExpression(Box<(Expression, Expression)>),
    RightShiftExpression(Box<(Expression, Expression)>),
    AndExpression(Box<(Expression, Expression)>),
    OrExpression(Box<(Expression, Expression)>),
    XorExpression(Box<(Expression, Expression)>),
    NegExpression(Box<Expression>),
    NotExpression(Box<Expression>),
}

impl TokenField {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> TokenField {
        let big_endian = elem.as_bool_or(AttributeId::Bigendian, false);
        let sign_bit = elem.as_bool_or(AttributeId::Signbit, false);
        let bit_start = elem.as_int_or(AttributeId::Startbit, 0) as i32;
        let bit_end = elem.as_int_or(AttributeId::Endbit, 0) as i32;
        let byte_start = elem.as_int_or(AttributeId::Startbyte, 0) as i32;
        let byte_end = elem.as_int_or(AttributeId::Endbyte, 0) as i32;
        let shift = elem.as_int_or(AttributeId::Shift, 0) as i32;
        reader.seek_elem_children_start(elem);

        reader.read_elem_end(elem.id);
        TokenField {
            big_endian,
            sign_bit,
            bit_start,
            bit_end,
            byte_start,
            byte_end,
            shift,
        }
    }

    // todo: should return 0 for unread bytes, not the whole thing
    pub fn evaluate(&self, state: &DisasmState, at: u64) -> i64 {
        // lazy tn, pls fix later
        let byte_count = self.byte_end - self.byte_start + 1;
        let bit_count = (self.bit_end + self.byte_end * 8) - (self.bit_start + self.byte_start * 8) + 1;

        let read_value =
            match state.read_mem_u64_bits_at(at + self.byte_start as u64, 0, byte_count * 8, self.big_endian) {
                Ok(v) => v,
                Err(_) => 0,
            };
        let mut value: i64 = (read_value >> self.shift) as i64;

        value &= (1 << bit_count) - 1;
        if self.sign_bit && value >= 0 && (value & (1 << (bit_count - 1))) != 0 {
            // manually sign extend
            let mask = 1 << (bit_count - 1);
            value = (value ^ mask) - mask;
        }
        value
    }
}

impl ContextField {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> ContextField {
        let sign_bit = elem.as_bool_or(AttributeId::Signbit, false);
        let bit_start = elem.as_int_or(AttributeId::Startbit, 0) as i32;
        let bit_end = elem.as_int_or(AttributeId::Endbit, 0) as i32;
        let byte_start = elem.as_int_or(AttributeId::Startbyte, 0) as i32;
        let byte_end = elem.as_int_or(AttributeId::Endbyte, 0) as i32;
        let shift = elem.as_int_or(AttributeId::Shift, 0) as i32;
        reader.seek_elem_children_start(elem);

        reader.read_elem_end(elem.id);
        ContextField {
            sign_bit,
            bit_start,
            bit_end,
            byte_start,
            byte_end,
            shift,
        }
    }

    pub fn evaluate(&self, state: &DisasmState) -> i64 {
        // lazy tn, pls fix later
        let byte_count = self.byte_end - self.byte_start + 1;
        let bit_count = (self.bit_end + self.byte_end * 8) - (self.bit_start + self.byte_start * 8) + 1;

        let read_value = state.read_ctx_u32_bits_at(self.byte_start * 8, byte_count * 8);
        let mut value: i64 = (read_value >> self.shift) as i64;

        value &= (1 << bit_count) - 1;
        if self.sign_bit && value >= 0 && (value & (1 << (bit_count - 1))) != 0 {
            value = -value;
        }
        value
    }
}

impl OperandValue {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> OperandValue {
        let index = elem.as_int_or(AttributeId::Index, 0) as i32;
        let sym_id = elem.as_uint_or(AttributeId::Table, 0) as u32;
        let ctor_idx = elem.as_uint_or(AttributeId::Ct, 0) as u32;
        reader.seek_elem_children_start(elem);

        reader.read_elem_end(elem.id);
        OperandValue {
            index,
            sym_id,
            ctor_idx,
        }
    }

    pub fn evaluate(&self, disasm: &Disasm, state: &DisasmState, top_stack: &DisasmOperandStackItem) -> i64 {
        if self.sym_id != top_stack.subsym_id || self.ctor_idx != top_stack.ctor_idx {
            unimplemented!("can't read a constructor operand that's outside the current one");
        }

        let sleigh_symbols = &disasm.sleigh.symbol_table.symbols;

        let operand_id = top_stack.operand_ids[self.index as usize];
        let operand_sym_box = &sleigh_symbols[operand_id as usize];
        let operand_sym = if let SymbolInner::OperandSym(v) = &operand_sym_box.inner {
            v
        } else {
            panic!("not an operand symbol")
        };

        // since we shouldn't expect much nesting for operands,
        // we don't build another stack and just use recursion
        let operand_off = top_stack.read_position + operand_sym.rel_offset as u64;
        let subsym_idx = operand_sym.subsym;
        if subsym_idx != u32::MAX {
            // dynamic value
            let operand_subsym_box = &sleigh_symbols[subsym_idx as usize];
            return match &operand_subsym_box.inner {
                SymbolInner::ValueSym(value_sym) => value_sym.patexp.evaluate(disasm, state, top_stack, operand_off),
                SymbolInner::VarlistSym(varlist_sym) => {
                    varlist_sym.patexp.evaluate(disasm, state, top_stack, operand_off)
                }
                SymbolInner::ValuemapSym(valuemap_sym) => {
                    valuemap_sym.patexp.evaluate(disasm, state, top_stack, operand_off)
                }
                SymbolInner::SubtableSym(_) => {
                    panic!("subtable can't be used with pattern expressions")
                }
                _ => panic!("unsupported symbol type for operand"),
            };
        } else if let Some(def_exp) = &operand_sym.def_exp {
            // static value
            return def_exp.evaluate(disasm, state, top_stack, operand_off);
        } else {
            panic!("either operand subsymbol or defexp should've been set");
        }
    }
}

fn parse_constant_value(reader: &SlaBinReader, elem: &SlaElement) -> i64 {
    let value = elem.as_int_or(AttributeId::Val, 0);
    reader.seek_elem_children_start(elem);
    reader.read_elem_end(elem.id);
    value
}

fn parse_empty(reader: &SlaBinReader, elem: &SlaElement) {
    reader.seek_elem_children_start(elem);
    reader.read_elem_end(elem.id);
}

fn parse_single_exp(reader: &SlaBinReader, elem: &SlaElement) -> Box<Expression> {
    reader.seek_elem_children_start(elem);
    let mut child_iter = reader.read_elem_children(elem.epos);

    let exp_ele = child_iter.next().expect("expression missing");
    let ele = Expression::new(reader, &exp_ele);

    reader.read_elem_end(elem.id);
    Box::new(ele)
}

fn parse_tuple_exp(reader: &SlaBinReader, elem: &SlaElement) -> Box<(Expression, Expression)> {
    reader.seek_elem_children_start(elem);
    let mut child_iter = reader.read_elem_children(elem.epos);

    let left_ele = child_iter.next().expect("left expression missing");
    let left = Expression::new(reader, &left_ele);
    let right_ele = child_iter.next().expect("right expression missing");
    let right = Expression::new(reader, &right_ele);

    reader.read_elem_end(elem.id);
    Box::new((left, right))
}

impl Expression {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Expression {
        match elem.id {
            ElementId::Tokenfield => Expression::TokenField(Box::new(TokenField::new(reader, elem))),
            ElementId::Contextfield => Expression::ContextField(Box::new(ContextField::new(reader, elem))),
            ElementId::Intb => Expression::ConstantValue(parse_constant_value(reader, elem)),
            ElementId::OperandExp => Expression::OperandValue(Box::new(OperandValue::new(reader, elem))),
            ElementId::StartExp => {
                parse_empty(reader, elem);
                Expression::StartInstructionValue
            }
            ElementId::EndExp => {
                parse_empty(reader, elem);
                Expression::EndInstructionValue
            }
            ElementId::Next2Exp => {
                parse_empty(reader, elem);
                Expression::Next2InstructionValue
            }
            ElementId::PlusExp => Expression::AddExpression(parse_tuple_exp(reader, elem)),
            ElementId::SubExp => Expression::SubExpression(parse_tuple_exp(reader, elem)),
            ElementId::MultExp => Expression::MultExpression(parse_tuple_exp(reader, elem)),
            ElementId::DivExp => Expression::DivExpression(parse_tuple_exp(reader, elem)),
            ElementId::LshiftExp => Expression::LeftShiftExpression(parse_tuple_exp(reader, elem)),
            ElementId::RshiftExp => Expression::RightShiftExpression(parse_tuple_exp(reader, elem)),
            ElementId::AndExp => Expression::AndExpression(parse_tuple_exp(reader, elem)),
            ElementId::OrExp => Expression::OrExpression(parse_tuple_exp(reader, elem)),
            ElementId::XorExp => Expression::XorExpression(parse_tuple_exp(reader, elem)),
            ElementId::MinusExp => Expression::NegExpression(parse_single_exp(reader, elem)),
            ElementId::NotExp => Expression::NotExpression(parse_single_exp(reader, elem)),
            _ => {
                panic!("unsupported pattern expression type")
            }
        }
    }

    pub fn evaluate(&self, disasm: &Disasm, state: &DisasmState, top_stack: &DisasmOperandStackItem, at: u64) -> i64 {
        match self {
            Expression::TokenField(token_field) => token_field.evaluate(state, at),
            Expression::ContextField(context_field) => context_field.evaluate(state),
            Expression::ConstantValue(constant_value) => *constant_value,
            Expression::OperandValue(operand_value) => {
                // since this is a reference to a constructor's operand value,
                // we have no value to read at the current position, so at isn't needed
                operand_value.evaluate(disasm, state, top_stack)
            }
            Expression::StartInstructionValue => state.get_start_ins(),
            Expression::EndInstructionValue => state.get_end_ins(),
            Expression::Next2InstructionValue => state.get_next2_ins(),
            Expression::AddExpression(sub_exp) => {
                let left = sub_exp.0.evaluate(disasm, state, top_stack, at);
                let right = sub_exp.1.evaluate(disasm, state, top_stack, at);
                left + right
            }
            Expression::SubExpression(sub_exp) => {
                let left = sub_exp.0.evaluate(disasm, state, top_stack, at);
                let right = sub_exp.1.evaluate(disasm, state, top_stack, at);
                left - right
            }
            Expression::MultExpression(mul_exp) => {
                let left = mul_exp.0.evaluate(disasm, state, top_stack, at);
                let right = mul_exp.1.evaluate(disasm, state, top_stack, at);
                left * right
            }
            Expression::DivExpression(div_exp) => {
                let left = div_exp.0.evaluate(disasm, state, top_stack, at);
                let right = div_exp.1.evaluate(disasm, state, top_stack, at);
                left / right
            }
            Expression::LeftShiftExpression(ls_exp) => {
                let left = ls_exp.0.evaluate(disasm, state, top_stack, at);
                let right = ls_exp.1.evaluate(disasm, state, top_stack, at);
                left << right
            }
            Expression::RightShiftExpression(rs_exp) => {
                let left = rs_exp.0.evaluate(disasm, state, top_stack, at);
                let right = rs_exp.1.evaluate(disasm, state, top_stack, at);
                left >> right
            }
            Expression::AndExpression(and_exp) => {
                let left = and_exp.0.evaluate(disasm, state, top_stack, at);
                let right = and_exp.1.evaluate(disasm, state, top_stack, at);
                left & right
            }
            Expression::OrExpression(or_exp) => {
                let left = or_exp.0.evaluate(disasm, state, top_stack, at);
                let right = or_exp.1.evaluate(disasm, state, top_stack, at);
                left | right
            }
            Expression::XorExpression(xor_exp) => {
                let left = xor_exp.0.evaluate(disasm, state, top_stack, at);
                let right = xor_exp.1.evaluate(disasm, state, top_stack, at);
                left ^ right
            }
            Expression::NegExpression(neg_exp) => -neg_exp.evaluate(disasm, state, top_stack, at),
            Expression::NotExpression(not_exp) => !not_exp.evaluate(disasm, state, top_stack, at),
        }
    }
}
