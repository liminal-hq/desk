/* The core's side of the module boundary: the callbacks a module may call, and the two ways they reach it.
 *
 * (c) Copyright 2026 Liminal HQ, Scott Morris
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */
#ifndef DESK_SPIKE_API_H
#define DESK_SPIKE_API_H

#include <stddef.h>
#include <sys/dxe.h>

/* One event, passed to a module by pointer (docs/SDK.md). */
typedef struct {
    unsigned kind, window, control;
    int x, y;
    unsigned key, mods, data;
} DeskEvent;

/* Set to silence desk_log (the loops that run a module hundreds of times). */
extern int desk_quiet;

/* Route a: the callbacks by name. They are what a module imports; desk_exports is what dlregsym is given. */
void desk_log(const char *text, size_t len);
void *desk_alloc(size_t size);
void desk_free(void *ptr);
unsigned desk_window_new(unsigned app, const char *title, unsigned len, int w, int h, unsigned flags);
void desk_panic(const char *msg, size_t len) __attribute__((noreturn));
/* Registers (or removes) the table of the above with dlregsym, so dlopen can resolve a module's imports from it. */
int desk_register_exports(void);
int desk_unregister_exports(void);

/* Route b: the same callbacks (and the memory routines the compiler calls) in a table passed to app_start. */
typedef struct {
    unsigned version, size;
    void (*log)(const char *, size_t);
    void *(*alloc)(size_t);
    void (*free)(void *);
    unsigned (*window_new)(unsigned, const char *, unsigned, int, int, unsigned);
    void (*panic)(const char *, size_t) __attribute__((noreturn));
    void *(*memcpy)(void *, const void *, size_t);
    void *(*memset)(void *, int, size_t);
    void *(*memmove)(void *, const void *, size_t);
    int (*memcmp)(const void *, const void *, size_t);
} DeskApi;
extern const DeskApi desk_api;

/* The module whose code is running, so that allocations are charged to it and a panic or fault is blamed on it. */
struct Module;
extern struct Module *desk_current;

/* Per-module allocation accounting. */
void desk_allocs_reclaim(struct Module *m, unsigned *blocks, unsigned *bytes);
unsigned desk_allocs_live(const struct Module *m);

#endif
