/* The callbacks: logging, a tracked allocator, a stub window, the export table for route a and the function table for route b.
 *
 * (c) Copyright 2026 Liminal HQ, Scott Morris
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "api.h"
#include "mod.h"
#include "out.h"

/* Every block carries a header that links it into its module's list, so the core can count what a module holds and free what it
 * leaves behind. The header is 16 bytes so the block keeps malloc's 8-byte alignment. */
typedef struct Alloc {
    struct Alloc *next, *prev;
    Module *owner;
    size_t size;
} Alloc;

struct Module *desk_current;
int desk_quiet;

void desk_log(const char *text, size_t len)
{
    char line[256];

    if (desk_quiet)
        return;
    snprintf(line, sizeof line, "  log> %.*s", (int)len, text);
    desk_emit(line);
}

void *desk_alloc(size_t size)
{
    Alloc *a = malloc(sizeof(Alloc) + size);

    if (a == NULL)
        return NULL;
    a->owner = desk_current;
    a->size = size;
    a->prev = NULL;
    a->next = NULL;
    if (a->owner != NULL) {
        a->next = a->owner->allocs;
        if (a->next != NULL)
            a->next->prev = a;
        a->owner->allocs = a;
    }
    return a + 1;
}

static void unlink_alloc(Alloc *a)
{
    if (a->prev != NULL)
        a->prev->next = a->next;
    else if (a->owner != NULL)
        a->owner->allocs = a->next;
    if (a->next != NULL)
        a->next->prev = a->prev;
}

void desk_free(void *ptr)
{
    Alloc *a;

    if (ptr == NULL)
        return;
    a = (Alloc *)ptr - 1;
    unlink_alloc(a);
    free(a);
}

unsigned desk_allocs_live(const Module *m)
{
    unsigned n = 0;
    const Alloc *a;

    for (a = m->allocs; a != NULL; a = a->next)
        n++;
    return n;
}

void desk_allocs_reclaim(Module *m, unsigned *blocks, unsigned *bytes)
{
    Alloc *a = m->allocs;

    *blocks = 0;
    *bytes = 0;
    while (a != NULL) {
        Alloc *next = a->next;

        (*blocks)++;
        *bytes += (unsigned)a->size;
        free(a);
        a = next;
    }
    m->allocs = NULL;
}

unsigned desk_window_new(unsigned app, const char *title, unsigned len, int w, int h, unsigned flags)
{
    static unsigned next = 1;
    char line[128];

    (void)flags;
    snprintf(line, sizeof line, "  window> app %u opens '%.*s' %dx%d as window %u", app, (int)len, title, w, h, next);
    if (!desk_quiet)
        desk_emit(line);
    return next++;
}

/* desk_panic lives in mod.c: it must longjmp to the frame that called into the module. */

/* Route a. memcpy and memset are the C library's: the compiler emits calls to them. */
DXE_EXPORT_TABLE(desk_export_table)
    DXE_EXPORT(desk_log)
    DXE_EXPORT(desk_alloc)
    DXE_EXPORT(desk_free)
    DXE_EXPORT(desk_window_new)
    DXE_EXPORT(desk_panic)
    DXE_EXPORT(memcpy)
    DXE_EXPORT(memset)
    DXE_EXPORT(memmove)
    DXE_EXPORT(memcmp)
DXE_EXPORT_END

int desk_register_exports(void)
{
    return dlregsym(desk_export_table);
}

int desk_unregister_exports(void)
{
    return dlunregsym(desk_export_table);
}

/* Route b. */
const DeskApi desk_api = {
    1, sizeof(DeskApi), desk_log, desk_alloc, desk_free, desk_window_new, desk_panic, memcpy, memset, memmove, memcmp,
};
