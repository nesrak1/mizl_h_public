use super::disasm::DisasmState;
use crate::sleigh::consts::{AttributeId, ElementId};
use crate::sleigh::sla_reader::{SlaBinReader, SlaElement};
use num_traits::pow;

pub struct PatBlock {
    pub offset: i32,
    pub non_zero: i32,
    pub mask_value_pairs: Vec<(u32, u32)>,
}

pub enum DisjointPatternType {
    InstructionPattern,
    ContextPattern,
    CombinePattern,
}

pub struct DisjointPattern {
    pub pat_type: DisjointPatternType,
    pub pat_blocks: Vec<PatBlock>,
}

impl DisjointPattern {
    pub fn is_match(&self, state: &DisasmState, at: u64) -> bool {
        match self.pat_type {
            DisjointPatternType::InstructionPattern => self.check_ins_pattern(state, at, 0),
            DisjointPatternType::ContextPattern => self.check_ctx_pattern(state, 0),
            DisjointPatternType::CombinePattern => {
                let mut is_match = true;
                is_match &= self.check_ctx_pattern(state, 0);
                is_match &= self.check_ins_pattern(state, at, 1);
                is_match
            }
        }
    }

    fn check_ins_pattern(&self, state: &DisasmState, at: u64, pat_block_idx: usize) -> bool {
        let pat_block = &self.pat_blocks[pat_block_idx];
        let mut offset = pat_block.offset;
        for pair in &pat_block.mask_value_pairs {
            let data = match state.read_mem_u32_at(at + offset as u64, true) {
                Ok(v) => v,
                Err(_) => return false,
            };
            if (data & pair.0) != pair.1 {
                return false;
            }
            offset += 4;
        }
        return true;
    }

    fn check_ctx_pattern(&self, state: &DisasmState, pat_block_idx: usize) -> bool {
        let pat_block = &self.pat_blocks[pat_block_idx];
        let mut offset = pat_block.offset;
        for pair in &pat_block.mask_value_pairs {
            let data = state.read_ctx_u32_at(offset as u64);
            if (data & pair.0) != pair.1 {
                return false;
            }
            offset += 4;
        }
        return true;
    }
}

pub struct DecisionPair {
    pub ctor_id: i32,
    pub pattern: DisjointPattern,
}

pub struct Decision {
    pub context: bool,
    pub start: i32,
    pub size: i32,
    pub children: Vec<Decision>,
    pub pairs: Vec<DecisionPair>,
}

impl PatBlock {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> PatBlock {
        let offset = elem.as_int_or(AttributeId::Off, -1) as i32;
        let non_zero = elem.as_int_or(AttributeId::Nonzero, -1) as i32;
        reader.seek_elem_children_start(elem);

        let mut mask_value_pairs: Vec<(u32, u32)> = Vec::new();
        for child in reader.read_elem_children(elem.epos) {
            if child.id != ElementId::MaskWord {
                panic!("only expected mask word in patblock's children");
            }

            let mask = child.as_uint_or(AttributeId::Mask, u32::MAX as u64) as u32;
            let val = child.as_uint_or(AttributeId::Val, u32::MAX as u64) as u32;
            mask_value_pairs.push((mask, val));
            reader.read_elem_end(child.id);
        }

        reader.read_elem_end(elem.id);
        PatBlock {
            offset,
            non_zero,
            mask_value_pairs,
        }
    }
}

impl DisjointPattern {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> DisjointPattern {
        reader.seek_elem_children_start(elem);

        let pat_type;
        let mut pat_blocks;
        if elem.id == ElementId::CombinePat {
            pat_blocks = Vec::with_capacity(2);
            pat_type = DisjointPatternType::CombinePattern;

            let context_elem = reader.read_elem();
            reader.seek_elem_children_start(&context_elem);
            let context_pat_block_elem = reader.read_elem();
            let ctx_pat_block = PatBlock::new(reader, &context_pat_block_elem);
            pat_blocks.push(ctx_pat_block);
            reader.read_elem_end(context_elem.id);

            let instruct_elem = reader.read_elem();
            reader.seek_elem_children_start(&instruct_elem);
            let instruct_pat_block_elem = reader.read_elem();
            let ins_pat_block = PatBlock::new(reader, &instruct_pat_block_elem);
            pat_blocks.push(ins_pat_block);
            reader.read_elem_end(instruct_elem.id);
        } else {
            let pat_block_elem = reader.read_elem();
            if pat_block_elem.id == ElementId::PatBlock {
                pat_blocks = Vec::with_capacity(1);
                pat_type = match elem.id {
                    ElementId::InstructPat => DisjointPatternType::InstructionPattern,
                    ElementId::ContextPat => DisjointPatternType::ContextPattern,
                    _ => panic!("unsupported pattern type"),
                };
                let pat_block = PatBlock::new(reader, &pat_block_elem);
                pat_blocks.push(pat_block);
            } else {
                panic!("expected disjoint pattern child to be a pat block");
            }
        }

        reader.read_elem_end(elem.id);
        DisjointPattern { pat_type, pat_blocks }
    }
}

impl DecisionPair {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> DecisionPair {
        let ctor_id = elem.as_int_or(AttributeId::Id, -1) as i32;
        reader.seek_elem_children_start(elem);

        let pattern_elem = reader.read_elem();
        let pattern = DisjointPattern::new(reader, &pattern_elem);

        reader.read_elem_end(elem.id);
        DecisionPair { ctor_id, pattern }
    }
}

impl Decision {
    pub fn new(reader: &SlaBinReader, elem: &SlaElement) -> Decision {
        let context = elem.as_bool_or(AttributeId::Context, false);
        let start = elem.as_int_or(AttributeId::Startbit, -1) as i32;
        let size = elem.as_int_or(AttributeId::Size, -1) as i32;
        reader.seek_elem_children_start(elem);

        let mut children: Vec<Decision> = Vec::new();
        let mut pairs: Vec<DecisionPair> = Vec::new();

        for child in reader.read_elem_children(elem.epos) {
            if child.id == ElementId::Decision {
                assert_ne!(size, 0);
                children.push(Decision::new(reader, &child));
            } else if child.id == ElementId::Pair {
                assert_eq!(size, 0);
                pairs.push(DecisionPair::new(reader, &child));
            }
        }

        assert!(size == 0 || pow(2, size as usize) == children.len());

        reader.read_elem_end(elem.id);
        Decision {
            context,
            start,
            size,
            children,
            pairs,
        }
    }
}
