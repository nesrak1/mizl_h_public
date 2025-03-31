use crate::sleigh::consts::{AttributeId};
use crate::sleigh::expression::Expression;
use crate::sleigh::sla_reader::{SlaBinReader, SlaElement};
use super::sla_file::{SymbolInner, Symbol};

pub struct OperandSym {
    pub hand: i32,
    pub rel_offset: i32,
    pub offset_base: i32,
    pub min_length: i32,
    pub subsym: u32,
    pub code: bool,
    pub local_exp: Expression,
    pub def_exp: Option<Expression>,
}

impl OperandSym {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Symbol {
        let name = elem.as_str_or(AttributeId::Name, "");
        let id = elem.as_uint_or(AttributeId::Id, 0) as u32;
        let scope = elem.as_uint_or(AttributeId::Scope, 0) as u32;
        let hand = elem.as_int_or(AttributeId::Index, 0) as i32;
        let rel_offset = elem.as_int_or(AttributeId::Off, 0) as i32;
        let offset_base = elem.as_int_or(AttributeId::Base, 0) as i32;
        let min_length = elem.as_int_or(AttributeId::Minlen, 0) as i32;
        let subsym = elem.as_uint_or(AttributeId::Subsym, u32::MAX as u64) as u32;
        let code = elem.as_bool_or(AttributeId::Code, false);
        reader.seek_elem_children_start(elem);

        let mut child_iter = reader.read_elem_children(elem.epos);

        let local_exp_ele = child_iter.next().expect("local operand expression missing");
        let local_exp = Expression::new(reader, &local_exp_ele);

        let mut def_exp = None;
        if subsym == u32::MAX {
            let def_exp_ele = child_iter.next().expect("def operand expression missing");
            def_exp = Some(Expression::new(reader, &def_exp_ele));
        }

        //reader.seek_elem_children_end(elem);
        reader.read_elem_end(elem.id);
        Symbol {
            name,
            id,
            scope,
            inner: SymbolInner::OperandSym(Box::new(OperandSym {
                hand,
                rel_offset,
                offset_base,
                min_length,
                subsym,
                code,
                local_exp,
                def_exp,
            })),
        }
    }
}
