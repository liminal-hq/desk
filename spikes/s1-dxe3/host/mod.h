/* Loading, calling and unloading a module, with a setjmp frame around every call into it.
 *
 * (c) Copyright 2026 Liminal HQ, Scott Morris
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */
#ifndef DESK_SPIKE_MOD_H
#define DESK_SPIKE_MOD_H

#include "api.h"

struct Alloc;

typedef enum { MOD_UNLOADED, MOD_LOADED, MOD_RUNNING, MOD_CRASHED } ModState;

typedef struct Module {
    char name[16];
    unsigned app;
    char route; /* 'a' or 'b': how the module reaches the core */
    ModState state;
    void *handle;
    int (*start)();
    void (*event)(unsigned, const DeskEvent *);
    void (*stop)(unsigned);
    struct Alloc *allocs; /* blocks the module holds, through desk_alloc */
    char reason[200];     /* why it crashed */
} Module;

/* 1: dlopen with RTLD_GLOBAL, so the module's exports join the global symbol space (the default is RTLD_LOCAL). */
extern int mod_global;

/* dlopen the file and look up the three entry points. Returns 0, or -1 with the dlerror text printed. */
int mod_load(Module *m, const char *name, const char *file, unsigned app, char route);
/* Each returns 0, or -1 if the module crashed (it is then marked MOD_CRASHED and still has to be unloaded). */
int mod_start(Module *m, const char *args);
int mod_event(Module *m, const DeskEvent *e);
int mod_stop(Module *m);
/* dlclose, free whatever the module still holds, and say how much that was. */
void mod_unload(Module *m);

/* Faults: the signals a module's CPU exception turns into. Returns after installing handlers that recover into the same path as a
 * panic. */
void mod_catch_faults(int on);

#endif
