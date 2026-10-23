/* Module lifecycle: dlopen, dlsym, guarded calls (setjmp) and the recovery from a panic or a CPU fault (longjmp).
 *
 * (c) Copyright 2026 Liminal HQ, Scott Morris
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */
#include <dlfcn.h>
#include <setjmp.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mod.h"
#include "out.h"

static jmp_buf guard;
int mod_global;

/* A Rust panic in a module calls this (route a: imported; route b: through the table). It never returns: control goes back to the
 * mod_guarded frame that is running the module's call. */
void desk_panic(const char *msg, size_t len)
{
    if (desk_current == NULL) {
        desk_emit("host: panic outside a module call");
        exit(4);
    }
    snprintf(desk_current->reason, sizeof desk_current->reason, "panic: %.*s", (int)len, msg);
    longjmp(guard, 1);
}

static void on_fault(int sig)
{
    const char *what = sig == SIGSEGV ? "SIGSEGV" : sig == SIGFPE ? "SIGFPE" : sig == SIGILL ? "SIGILL" : "signal";

    if (desk_current == NULL) {
        desk_emit("host: fault outside a module call");
        exit(5);
    }
    snprintf(desk_current->reason, sizeof desk_current->reason, "fault: %s (%d)", what, sig);
    signal(sig, on_fault); /* DJGPP resets the handler to the default after delivery */
    longjmp(guard, 2);
}

void mod_catch_faults(int on)
{
    void (*h)(int) = on ? on_fault : SIG_DFL;

    signal(SIGSEGV, h);
    signal(SIGFPE, h);
    signal(SIGILL, h);
}

int mod_load(Module *m, const char *name, const char *file, unsigned app, char route)
{
    char line[160];

    memset(m, 0, sizeof *m);
    snprintf(m->name, sizeof m->name, "%s", name);
    m->app = app;
    m->route = route;
    m->handle = dlopen(file, RTLD_LAZY | (mod_global ? RTLD_GLOBAL : 0));
    if (m->handle == NULL) {
        snprintf(line, sizeof line, "host: dlopen %s failed: %s", file, dlerror());
        desk_emit(line);
        return -1;
    }
    m->start = (int (*)())dlsym(m->handle, "_app_start");
    m->event = (void (*)(unsigned, const DeskEvent *))dlsym(m->handle, "_app_event");
    m->stop = (void (*)(unsigned))dlsym(m->handle, "_app_stop");
    if (m->start == NULL || m->event == NULL || m->stop == NULL) {
        snprintf(line, sizeof line, "host: dlsym in %s failed: %s", file, dlerror());
        desk_emit(line);
        dlclose(m->handle);
        m->handle = NULL;
        return -1;
    }
    m->state = MOD_LOADED;
    return 0;
}

typedef enum { CALL_START, CALL_EVENT, CALL_STOP } Call;

/* Every call into a module goes through here. setjmp sits in this frame; a panic or a fault in the module longjmps back to it, so
 * the module's own frames are simply discarded. The core's frames are never between the two, because a module only calls
 * callbacks that return. */
static int mod_guarded(Module *m, Call what, const char *args, const DeskEvent *e)
{
    volatile int result = 0;

    desk_current = m;
    if (setjmp(guard) == 0) {
        switch (what) {
        case CALL_START:
            if (m->route == 'b')
                result = m->start(m->app, args, (unsigned)strlen(args), &desk_api);
            else
                result = m->start(m->app, args, (unsigned)strlen(args));
            break;
        case CALL_EVENT:
            m->event(m->app, e);
            break;
        case CALL_STOP:
            m->stop(m->app);
            break;
        }
        desk_current = NULL;
        return result;
    }
    desk_current = NULL;
    m->state = MOD_CRASHED;
    {
        char line[256];

        snprintf(line, sizeof line, "host: module %s crashed: %s", m->name, m->reason);
        if (!desk_quiet)
            desk_emit(line);
    }
    return -1;
}

int mod_start(Module *m, const char *args)
{
    int r = mod_guarded(m, CALL_START, args, NULL);

    if (r == 0 && m->state == MOD_LOADED)
        m->state = MOD_RUNNING;
    return r;
}

int mod_event(Module *m, const DeskEvent *e)
{
    return mod_guarded(m, CALL_EVENT, NULL, e);
}

int mod_stop(Module *m)
{
    return mod_guarded(m, CALL_STOP, NULL, NULL);
}

void mod_unload(Module *m)
{
    unsigned blocks, bytes;
    int r;
    char line[160];

    r = dlclose(m->handle);
    desk_allocs_reclaim(m, &blocks, &bytes);
    snprintf(line, sizeof line, "host: unloaded %s (dlclose %d, reclaimed %u blocks, %u bytes)", m->name, r, blocks, bytes);
    if (!desk_quiet)
        desk_emit(line);
    m->handle = NULL;
    m->state = MOD_UNLOADED;
}
