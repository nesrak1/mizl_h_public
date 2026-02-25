use super::sla_file::{Symbol, SymbolInner};
use crate::sleigh::consts::{AttributeId, ElementId};
use crate::sleigh::expression::Expression;
use crate::sleigh::sla_reader::{SlaBinReader, SlaElement};

pub struct ValuemapSym {
    pub patexp: Expression,
    pub values: Vec<i64>,
}

impl ValuemapSym {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Symbol {
        let name = elem.as_str_or(AttributeId::Name, "");
        let id = elem.as_uint_or(AttributeId::Id, 0) as u32;
        let scope = elem.as_uint_or(AttributeId::Scope, 0) as u32;
        reader.seek_elem_children_start(elem);

        let mut patexp: Option<Expression> = None;
        let mut values: Vec<i64> = Vec::new();
        for child in reader.read_elem_children(elem.epos) {
            if patexp.is_none() {
                patexp = Some(Expression::new(reader, &child));
                continue;
            }
            if child.id == ElementId::Valuetab {
                let val = child.as_int_or(AttributeId::Val, 0xBADBEEF);
                values.push(val);
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
            inner: SymbolInner::ValuemapSym(Box::new(ValuemapSym {
                patexp: patexp.unwrap(),
                values,
            })),
        }
    }
}
