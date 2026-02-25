pub struct GbfNodeKind;
impl GbfNodeKind {
    // GhidraClassName - GHIDRA_ORIGINAL_CONST_NAME
    pub const LONGKEY_INTERIOR: u8 = 0; // LongKeyInteriorNode - LONGKEY_INTERIOR_NODE
    pub const LONGKEY_VAR_REC: u8 = 1; // VarRecNode - LONGKEY_VAR_REC_NODE
    pub const LONGKEY_FIXED_REC: u8 = 2; // FixedRecNode - LONGKEY_FIXED_REC_NODE
    pub const VARKEY_INTERIOR: u8 = 3; // VarKeyInteriorNode - VARKEY_INTERIOR_NODE
    pub const VARKEY_REC: u8 = 4; // VarKeyRecordNode - VARKEY_REC_NODE
    pub const FIXEDKEY_INTERIOR: u8 = 5; // FixedKeyInteriorNode - FIXEDKEY_INTERIOR_NODE
    pub const FIXEDKEY_VAR_REC: u8 = 6; // FixedKeyVarRecNode - FIXEDKEY_VAR_REC_NODE
    pub const FIXEDKEY_FIXED_REC: u8 = 7; // FixedKeyFixedRecNode - FIXEDKEY_FIXED_REC_NODE
    pub const CHAINED_BUFFER_INDEX: u8 = 8;
    pub const CHAINED_BUFFER_DATA: u8 = 9;
}
