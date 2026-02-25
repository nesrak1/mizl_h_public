use crate::sleigh::consts::{AttributeId, ElementId};
use crate::sleigh::expression::Expression;
use crate::sleigh::sla_reader::{SlaBinReader, SlaElement};
use super::sla_file::{SymbolInner, Symbol};

pub struct VarlistSym {
    pub patexp: Expression,
    pub var_ids: Vec<u32>,
}

impl VarlistSym {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Symbol {
        let name = elem.as_str_or(AttributeId::Name, "");
        let id = elem.as_uint_or(AttributeId::Id, 0) as u32;
        let scope = elem.as_uint_or(AttributeId::Scope, 0) as u32;
        reader.seek_elem_children_start(elem);

        let mut patexp: Option<Expression> = None;
        let mut var_ids: Vec<u32> = Vec::new();
        for child in reader.read_elem_children(elem.epos) {
            if patexp.is_none() {
                patexp = Some(Expression::new(reader, &child));
                continue;
            }
            if child.id == ElementId::Var {
                let id = child.as_uint_or(AttributeId::Id, u32::MAX as u64) as u32;
                var_ids.push(id);
                reader.read_elem_end(child.id);
            } else if child.id == ElementId::Null {
                var_ids.push(u32::MAX);
                reader.read_elem_end(child.id);
            }
        }

        if patexp.is_none() {
            panic!("pattern expression missing");
        }

        reader.read_elem_end(elem.id);
        Symbol {
            name,
            id,
            scope,
            inner: SymbolInner::VarlistSym(Box::new(VarlistSym {
                patexp: patexp.unwrap(),
                var_ids,
            })),
        }
    }
}