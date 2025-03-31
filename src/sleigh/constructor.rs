use crate::sleigh::consts::{AttributeId, ElementId};
use crate::sleigh::expression::Expression;
use crate::sleigh::sla_reader::{SlaBinReader, SlaElement, SpaceInfo};

pub enum ConstructorPrintElement {
    Operand(i32),
    Literal(String),
}

pub struct ConstructorTpl {
    pub labels: i32,
    pub section: i32,
    pub result: Option<HandleTpl>,
    pub op_tpls: Vec<OpTpl>,
}

pub struct ContextOpTpl {
    pub word_start: i32,
    pub bit_shift: i32,
    pub mask: u32,
    pub expression: Expression,
}

pub struct Constructor {
    pub parent: u32,
    pub first: i32,
    pub min_length: i32,
    pub source: i32,
    pub line: i32,
    pub operand_ids: Vec<u32>,
    pub print_elements: Vec<ConstructorPrintElement>,
    pub context_ops: Vec<ContextOpTpl>,
    pub template: Option<ConstructorTpl>,
}

pub struct HandleTpl {
    pub space: ConstTpl,
    pub size: ConstTpl,
    pub ptrspace: ConstTpl,
    pub ptroffset: ConstTpl,
    pub ptrsize: ConstTpl,
    pub temp_space: ConstTpl,
    pub temp_offset: ConstTpl,
}

pub struct OpTpl {
    // opcode type
    pub code: i32,
    pub result: Option<VarNodeTpl>,
    pub input: Vec<VarNodeTpl>,
}

pub struct VarNodeTpl {
    pub space: ConstTpl,
    pub offset: ConstTpl,
    pub size: ConstTpl,
}

#[derive(FromPrimitive, Clone, Copy, PartialEq, Eq)]
pub enum ConstTplType {
    Real,
    Handle,
    JStart,
    JNext,
    JNext2,
    JCurspace,
    JCurspaceSize,
    Spaceid,
    JRelative,
    Flowref,
    JFlowrefSize,
    JFlowdest,
    JFlowdestSize,
}

#[derive(FromPrimitive, Clone, Copy, PartialEq, Eq)]
pub enum ConstTplHandleType {
    Space,
    Offset,
    Size,
    OffsetPlus,
}

pub struct ConstTpl {
    pub const_type: ConstTplType,
    pub value_spaceid: Option<SpaceInfo>,
    pub value_real: u64,
}

impl ConstTpl {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> ConstTpl {
        reader.seek_elem_children_start(elem);

        let const_type = match elem.id {
            ElementId::ConstReal => ConstTplType::Real,
            ElementId::ConstHandle => ConstTplType::Handle,
            ElementId::ConstStart => ConstTplType::JStart,
            ElementId::ConstNext => ConstTplType::JNext,
            ElementId::ConstNext2 => ConstTplType::JNext2,
            ElementId::ConstCurspace => ConstTplType::JCurspace,
            ElementId::ConstCurspaceSize => ConstTplType::JCurspaceSize,
            ElementId::ConstSpaceid => ConstTplType::Spaceid,
            ElementId::ConstRelative => ConstTplType::JRelative,
            ElementId::ConstFlowref => ConstTplType::Flowref,
            ElementId::ConstFlowrefSize => ConstTplType::JFlowrefSize,
            ElementId::ConstFlowdest => ConstTplType::JFlowdest,
            ElementId::ConstFlowdestSize => ConstTplType::JFlowdestSize,
            _ => panic!("unsupported const template type"),
        };
        let res: ConstTpl;
        if const_type == ConstTplType::Real || const_type == ConstTplType::JRelative {
            let value = elem.as_uint_or(AttributeId::Val, 0);
            res = ConstTpl {
                const_type,
                value_spaceid: None,
                value_real: value,
            };
        } else if const_type == ConstTplType::Handle {
            _ = elem.as_int_or(AttributeId::Val, 0) as i16; // handle_index
            let select = elem.as_int_or(AttributeId::S, 0) as i16;
            let value: u64;
            if select == ConstTplHandleType::OffsetPlus as i16 {
                value = elem.as_uint_or(AttributeId::Plus, 0);
            } else {
                value = 0;
            }
            res = ConstTpl {
                const_type,
                value_spaceid: None,
                value_real: value,
            };
        } else if const_type == ConstTplType::Spaceid {
            let value = elem.as_space(AttributeId::Space);
            res = ConstTpl {
                const_type,
                value_spaceid: Some(value),
                value_real: 0,
            }
        } else {
            res = ConstTpl {
                const_type,
                value_spaceid: None,
                value_real: 0,
            }
        }

        reader.read_elem_end(elem.id);
        return res;
    }
}

impl VarNodeTpl {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> VarNodeTpl {
        reader.seek_elem_children_start(elem);

        let mut child_iter = reader.read_elem_children(elem.epos);
        let space_elem = child_iter.next().expect("space template missing");
        let space = ConstTpl::new(reader, &space_elem);
        let offset_elem = child_iter.next().expect("offset template missing");
        let offset = ConstTpl::new(reader, &offset_elem);
        let size_elem = child_iter.next().expect("size template missing");
        let size = ConstTpl::new(reader, &size_elem);
        assert!(child_iter.next().is_none());

        reader.read_elem_end(elem.id);
        VarNodeTpl { space, offset, size }
    }
}

impl OpTpl {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> OpTpl {
        let code = elem.as_int_or(AttributeId::Code, -1) as i32;
        reader.seek_elem_children_start(elem);

        let mut input: Vec<VarNodeTpl> = Vec::new();
        let mut result: Option<VarNodeTpl> = None;
        for (i, child) in reader.read_elem_children(elem.epos).enumerate() {
            if i == 0 {
                if child.id == ElementId::Null {
                    reader.read_elem_end(child.id);
                } else if child.id == ElementId::VarnodeTpl {
                    result = Some(VarNodeTpl::new(reader, &child));
                } else {
                    panic!("can't handle this result type");
                }
            } else {
                if child.id == ElementId::VarnodeTpl {
                    input.push(VarNodeTpl::new(reader, &child));
                } else {
                    panic!("can't handle this input type");
                }
            }
        }
        input.shrink_to_fit();

        reader.read_elem_end(elem.id);
        OpTpl { code, result, input }
    }
}

impl HandleTpl {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> HandleTpl {
        reader.seek_elem_children_start(elem);

        let mut child_iter = reader.read_elem_children(elem.epos);
        let space_elem = child_iter.next().expect("space missing");
        let space = ConstTpl::new(reader, &space_elem);
        let size_elem = child_iter.next().expect("size missing");
        let size = ConstTpl::new(reader, &size_elem);
        let ptrspace_elem = child_iter.next().expect("ptrspace missing");
        let ptrspace = ConstTpl::new(reader, &ptrspace_elem);
        let ptroffset_elem = child_iter.next().expect("ptroffset missing");
        let ptroffset = ConstTpl::new(reader, &ptroffset_elem);
        let ptrsize_elem = child_iter.next().expect("ptrsize missing");
        let ptrsize = ConstTpl::new(reader, &ptrsize_elem);
        let temp_space_elem = child_iter.next().expect("temp_space missing");
        let temp_space = ConstTpl::new(reader, &temp_space_elem);
        let temp_offset_elem = child_iter.next().expect("temp_offset missing");
        let temp_offset = ConstTpl::new(reader, &temp_offset_elem);

        reader.read_elem_end(elem.id);
        HandleTpl {
            space,
            size,
            ptrspace,
            ptroffset,
            ptrsize,
            temp_space,
            temp_offset,
        }
    }
}

impl ConstructorTpl {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> ConstructorTpl {
        let labels = elem.as_int_or(AttributeId::Labels, 0) as i32;
        let section = elem.as_int_or(AttributeId::Section, -1) as i32;
        reader.seek_elem_children_start(elem);

        let mut result: Option<HandleTpl> = None;
        let mut op_tpls: Vec<OpTpl> = Vec::new();
        for (i, child) in reader.read_elem_children(elem.epos).enumerate() {
            if i == 0 {
                if child.id == ElementId::Null {
                    reader.read_elem_end(child.id);
                } else if child.id == ElementId::HandleTpl {
                    result = Some(HandleTpl::new(reader, &child));
                } else {
                    panic!("can't handle this result type");
                }
            } else {
                if child.id == ElementId::OpTpl {
                    op_tpls.push(OpTpl::new(reader, &child));
                } else {
                    panic!("can't handle this input type");
                }
            }
        }
        op_tpls.shrink_to_fit();

        reader.read_elem_end(elem.id);
        ConstructorTpl {
            labels,
            section,
            result,
            op_tpls,
        }
    }
}

impl Constructor {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Constructor {
        let parent = elem.as_uint_or(AttributeId::Parent, 0) as u32;
        let first = elem.as_int_or(AttributeId::First, 0) as i32;
        let length = elem.as_int_or(AttributeId::Length, 0) as i32;
        let source = elem.as_int_or(AttributeId::Source, 0) as i32;
        let line = elem.as_int_or(AttributeId::Line, 0) as i32;
        reader.seek_elem_children_start(elem);

        let mut operand_ids: Vec<u32> = Vec::new();
        let mut print_elements: Vec<ConstructorPrintElement> = Vec::new();
        let mut context_ops: Vec<ContextOpTpl> = Vec::new();
        let mut template: Option<ConstructorTpl> = None;
        for child in reader.read_elem_children(elem.epos) {
            if child.id == ElementId::Oper {
                let operand_id = child.as_uint_or(AttributeId::Id, 0) as u32;
                operand_ids.push(operand_id);
                reader.read_elem_end(child.id);
            } else if child.id == ElementId::Print {
                let str = child.as_str_or(AttributeId::Piece, "");
                print_elements.push(ConstructorPrintElement::Literal(str));
                reader.read_elem_end(child.id);
            } else if child.id == ElementId::Opprint {
                let oper_index = child.as_int_or(AttributeId::Id, -1) as i32;
                print_elements.push(ConstructorPrintElement::Operand(oper_index));
                reader.read_elem_end(child.id);
            } else if child.id == ElementId::ConstructTpl {
                template = Some(ConstructorTpl::new(reader, &child));
            } else if child.id == ElementId::ContextOp {
                // todo
                let i = child.as_int_or(AttributeId::I, 0) as i32;
                let shift = child.as_int_or(AttributeId::Shift, 0) as i32;
                let mask = child.as_uint_or(AttributeId::Mask, 0) as u32;
                reader.seek_elem_children_start(&child);

                let mut ctx_child_iter = reader.read_elem_children(child.epos);
                let local_exp_ele = ctx_child_iter.next().expect("context op expression missing");
                let local_exp = Expression::new(reader, &local_exp_ele);
                reader.read_elem_end(child.id);
                context_ops.push(ContextOpTpl {
                    word_start: i,
                    bit_shift: shift,
                    mask,
                    expression: local_exp,
                });
            } else {
                panic!("unexpected child type while reading constructor");
            }
        }

        reader.read_elem_end(elem.id);
        Constructor {
            parent,
            first,
            min_length: length,
            source,
            line,
            operand_ids,
            print_elements,
            context_ops,
            template,
        }
    }
}
