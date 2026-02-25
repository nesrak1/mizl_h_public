#ifndef MIZL_COMMON_H
#define MIZL_COMMON_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define PhOpaque(x) x // this type is intentionally opaque

// PhObj
typedef struct
{
    // there is no alignment between length and data!
    // we guarantee - no matter the size of data -
    // that length starts four bytes before data.
    uint32_t length;
    uint8_t data;
} PhLData;

#define PhLDataStart(v) ((PhLData *)((char *)v - offsetof(PhLData, data)))
#define PhLDataLen(v) (PhLDataStart(v)->length)
#define PhLen(v) PhLDataLen(v)
#define PhVec(v) v *
#define PhStr char *

typedef struct
{
    uint32_t alignment;
    union
    {
        uint32_t size;
        int32_t error;
    };
    PhLData ldata;
} PhObj;

#define PhObjHeaderSize(v) (offsetof(PhObj, ldata) + offsetof(PhLData, data))
#define PhObjStart(v) ((PhObj *)((char *)v - PhObjHeaderSize(v)))
#define PhObjAlignment(v) (PhObjStart(v)->alignment)
#define PhObjSize(v) (PhObjStart(v)->size)
#define PhObjIsError(v) ((PhObjStart(v)->error) < 0)
#define PhObjError(v) (-(PhObjStart(v)->error) - 1)
#define PhObj(v) v      // denotes this object must be freed with pheap_free
#define PhObjMaybe(v) v // denotes PhObj but could be null even when there is no error
#define PhMaybe(v) v    // denotes non-PhObj but could be null even when there is no error

extern void pheap_free(void *obj);

#define PhErr(t) PhObj *

/*
examples
[-]: padding
[A]: alignment (used by rust for allocation)
[S]: size (used by rust for allocation)
[L]: length (optional length field if this is a list or string)
[D]: user data struct
*D]: pointer returned to user points to first user data bytes

assume D is a 2-byte aligned value
[A][A][A][A][S][S][S][S]
[L][L][L][L]*D][D]

assume D is a 4-byte aligned value
[A][A][A][A][S][S][S][S]
[L][L][L][L]*D][D][D][D]

assume D is a 8-byte aligned value
[-][-][-][-][A][A][A][A]
[S][S][S][S][L][L][L][L]
*D][D][D][D][D][D][D][D]

assume D is a 16-byte aligned value
[-][-][-][-][A][A][A][A]
[S][S][S][S][L][L][L][L]
*D][D][D][D][D][D][D][D]
[D][D][D][D][D][D][D][D]

assume D is a 32-byte aligned value (shouldn't normally happen, but let's assume)
[-][-][-][-][-][-][-][-]
[-][-][-][-][-][-][-][-]
[-][-][-][-][A][A][A][A]
[S][S][S][S][L][L][L][L]
*D][D][D][D][D][D][D][D]
[D][D][D][D][D][D][D][D]
[D][D][D][D][D][D][D][D]
[D][D][D][D][D][D][D][D]
*/

#endif // MIZL_COMMON_H