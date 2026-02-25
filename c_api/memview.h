#ifndef MIZL_MEMVIEW_H
#define MIZL_MEMVIEW_H

#include "common.h"

// memview
typedef enum
{
    MEM_VIEW_ERROR_END_OF_STREAM = 0,
    MEM_VIEW_ERROR_READ_ACCESS_DENIED = 1,
    MEM_VIEW_ERROR_WRITE_ACCESS_DENIED = 2,
    MEM_VIEW_ERROR_NOT_LOADED = 3,
    MEM_VIEW_ERROR_GENERIC = 4
} MemViewError;

typedef struct PhOpaque(MemView) MemView;

// ///////

PhObj(MemView *) static_mem_view_from_file(char *path, PhErr(MemViewError) * err);                             // #ctor
PhObj(MemView *) static_mem_view_from_data(unsigned char *data, uint64_t size, PhErr(MemViewError) * err_str); // #ctor

#endif // MIZL_MEMVIEW_H