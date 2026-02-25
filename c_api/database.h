#ifndef MIZL_DATABASE_H
#define MIZL_DATABASE_H

#include "common.h"
#include "memview.h"

// #-class GbfFieldKind
typedef enum
{
    GBFFIELDKIND_BYTE = 0,
    GBFFIELDKIND_SHORT = 1,
    GBFFIELDKIND_INT = 2,
    GBFFIELDKIND_LONG = 3,
    GBFFIELDKIND_STRING = 4,
    GBFFIELDKIND_BYTES = 5,
    GBFFIELDKIND_BOOLEAN = 6
} GbfFieldKind;

const char *GBF_FIELD_KIND_STR[] = {
    "Byte",
    "Short",
    "Int",
    "Long",
    "String",
    "Bytes",
    "Boolean"};

// #-class GbfDbParms
typedef struct
{
    uint8_t node_code;
    int32_t data_len;
    uint8_t version;
    PhVec(int32_t) values;
} GbfDbParms;

// #-class GbfRecord
typedef struct
{
    enum
    {
        GBFFIELDVALUE_TAG_BYTE = 0,
        GBFFIELDVALUE_TAG_SHORT = 1,
        GBFFIELDVALUE_TAG_INT = 2,
        GBFFIELDVALUE_TAG_LONG = 3,
        GBFFIELDVALUE_TAG_STRING = 4,
        GBFFIELDVALUE_TAG_BYTES = 5,
        GBFFIELDVALUE_TAG_BOOLEAN = 6
    } tag;
    union
    {
        bool vBoolean;
        int8_t vByte;
        int16_t vShort;
        int32_t vInt;
        int64_t vLong;
        PhStr vString;
        PhVec(uint8_t) vBytes;
    };
} GbfFieldValue;

typedef struct
{
    GbfFieldValue *key;
    PhVec(GbfFieldValue *) values;
} GbfRecord;

// #-opaques
typedef struct PhOpaque(GbfDatabase) GbfDatabase;
typedef struct PhOpaque(GbfTableDef) GbfTableDef;
typedef struct PhOpaque(GbfTableSchema) GbfTableSchema;
typedef struct PhOpaque(GbfTableView) GbfTableView;

// ///////

// #-class GbfDatabase
PhObj(GbfDatabase *) database_new(MemView *mv, uint64_t *at, PhErr(MemViewError) * err); // #ctor
PhObj(GbfDbParms *) database_get_db_parms(GbfDatabase *self, PhErr(MemViewError) * err);
PhMaybe(GbfTableDef *) database_get_table_def_by_name(GbfDatabase *self, char *table_name, PhErr(MemViewError) * err);
PhObj(PhVec(GbfTableDef *)) database_get_table_defs(GbfDatabase *self, PhErr(MemViewError) * err);
// PhObjMaybe(GbfTableView *) database_get_table_view_by_name(GbfDatabase *self, GbfTableSchema *schema, char *table_name, PhErr(MemViewError) * err);

// #-class GbfTableDef
GbfTableSchema *database_table_def_get_schema(GbfTableDef *self, PhErr(MemViewError) * err);
int32_t database_table_def_get_root_nid(GbfTableDef *self, PhErr(MemViewError) * err);

// #-class GbfTableSchema
PhObj(PhStr) database_table_schema_get_name(GbfTableSchema *self, PhErr(MemViewError) * err);
PhObj(PhStr) database_table_schema_get_key_name(GbfTableSchema *self, PhErr(MemViewError) * err);
GbfFieldKind database_table_schema_get_key_kind(GbfTableSchema *self, PhErr(MemViewError) * err);
PhObj(PhVec(GbfFieldKind)) database_table_schema_get_kinds(GbfTableSchema *self, PhErr(MemViewError) * err);
PhObj(PhVec(PhStr)) database_table_schema_get_names(GbfTableSchema *self, PhErr(MemViewError) * err);

// #-class GbfTableView
PhObj(GbfTableView *) database_view_new(GbfDatabase *gbf, GbfTableSchema *schema, int32_t root_nid, PhErr(MemViewError) * err);
PhObjMaybe(GbfRecord *) database_view_get_record_at_long(GbfTableView *self, int64_t key, PhErr(MemViewError) * err);
PhObjMaybe(GbfRecord *) database_view_get_record_after_long(GbfTableView *self, int64_t key, PhErr(MemViewError) * err);
PhObjMaybe(GbfRecord *) database_view_get_record_at_after_long(GbfTableView *self, int64_t key, PhErr(MemViewError) * err);

#endif // MIZL_DATABASE_H