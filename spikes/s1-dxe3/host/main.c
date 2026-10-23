/* The spike host: stands in for the core. HOST.EXE <scenario> runs one scenario and prints a transcript the checks compare.
 *
 * (c) Copyright 2026 Liminal HQ, Scott Morris
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */
#include <dpmi.h>
#include <go32.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "mod.h"
#include "out.h"

static void say(const char *fmt, ...)
{
    char line[256];
    va_list ap;

    va_start(ap, fmt);
    vsnprintf(line, sizeof line, fmt, ap);
    va_end(ap);
    desk_emit(line);
}

/* "mem:" lines hold numbers that change from run to run; the checks skip them and rely on the verdict lines instead. */
static void mem(const char *tag, unsigned long *virt, unsigned long *heap)
{
    *virt = _go32_dpmi_remaining_virtual_memory();
    *heap = (unsigned long)sbrk(0);
    say("mem: %-12s virtual=%lu physical=%lu brk=%lu", tag, *virt,
        (unsigned long)_go32_dpmi_remaining_physical_memory(), *heap);
}

static DeskEvent ev(unsigned kind, int x, int y)
{
    DeskEvent e;

    memset(&e, 0, sizeof e);
    e.kind = kind;
    e.x = x;
    e.y = y;
    return e;
}

static void send(Module *m, unsigned kind, int x, int y)
{
    DeskEvent e = ev(kind, x, y);

    mod_event(m, &e);
}

static const char *file_for(const char *base, char route, char *out)
{
    sprintf(out, "%s%s.DXE", base, route == 'b' ? "B" : "");
    return out;
}

static int open_module(Module *m, const char *base, unsigned app, char route, const char *args)
{
    char file[32];

    if (mod_load(m, base, file_for(base, route, file), app, route) != 0)
        return -1;
    return mod_start(m, args);
}

/* Start, a few events, stop, dlclose; twice, so a reload shows the module's statics starting over. */
static int scenario_life(char route)
{
    Module m;
    int round;

    desk_register_exports();
    for (round = 1; round <= 2; round++) {
        say("host: round %d, route %c", round, route);
        if (open_module(&m, "HELLO", 1, route, "alpha beta") != 0)
            return 1;
        send(&m, 5, 10, 20);
        send(&m, 6, 30, 40);
        send(&m, 7, 50, 60);
        mod_stop(&m);
        mod_unload(&m);
    }
    say("host: life ok");
    return 0;
}

/* Two different modules and a second copy of the first, all loaded at once, then the same file opened twice. */
static int scenario_two(char route)
{
    Module hello, counter, hello2, same;
    char f1[32], f2[32];

    desk_register_exports();
    if (open_module(&hello, "HELLO", 1, route, "one") != 0 || open_module(&counter, "COUNTER", 2, route, "") != 0 ||
        open_module(&hello2, "HELLO2", 3, route, "three") != 0)
        return 1;
    send(&hello, 1, 3, 0);
    send(&counter, 1, 3, 0);
    send(&hello2, 1, 9, 0);
    send(&hello, 2, 4, 0);
    send(&counter, 2, 4, 0);
    send(&counter, 3, 5, 0);
    mod_stop(&counter);
    mod_stop(&hello);
    mod_stop(&hello2);
    /* The same file a second time: does DXE3 give a second copy, with its own statics, or the first one again? */
    if (mod_load(&same, "HELLO-AGAIN", file_for("HELLO", route, f1), 4, route) == 0) {
        say("host: HELLO loaded again while open: %s handle", same.handle == hello.handle ? "the same" : "a different");
        mod_start(&same, "again");
        send(&same, 1, 1, 1);
        mod_stop(&same);
        mod_unload(&same);
    }
    (void)f2;
    mod_unload(&hello2);
    mod_unload(&counter);
    mod_unload(&hello);
    say("host: two ok");
    return 0;
}

/* A panic is recovered; a module loaded before it and one loaded after it keep working. */
static int scenario_panic(char route)
{
    Module before, counter, bad, bad2, after;

    desk_register_exports();
    if (open_module(&before, "HELLO", 1, route, "before") != 0 || open_module(&counter, "COUNTER", 2, route, "") != 0)
        return 1;
    send(&before, 1, 1, 1);
    send(&counter, 1, 8, 0);

    if (open_module(&bad, "CRASH", 3, route, "") != 0)
        return 1;
    send(&bad, 1, 0, 0);
    say("host: crash holds %u blocks before the fault", desk_allocs_live(&bad));
    send(&bad, 104, 0, 0); /* panics holding five heap blocks (and the message formatting's) */
    say("host: state %s", bad.state == MOD_CRASHED ? "crashed" : "not crashed");
    mod_unload(&bad);

    send(&before, 2, 2, 2);
    send(&counter, 2, 4, 0);
    if (open_module(&bad2, "CRASH", 4, route, "") != 0)
        return 1;
    send(&bad2, 100, 0, 0); /* a plain panic with a formatted message */
    mod_unload(&bad2);

    if (open_module(&after, "HELLO2", 5, route, "after") != 0)
        return 1;
    send(&after, 1, 7, 7);
    send(&before, 3, 3, 3);
    send(&counter, 3, 6, 0);
    mod_stop(&after);
    mod_unload(&after);
    mod_stop(&counter);
    mod_stop(&before);
    mod_unload(&counter);
    mod_unload(&before);
    say("host: panic ok, the core kept running");
    return 0;
}

/* A CPU fault in a module: null write, stack overflow or divide error, with or without the signal handlers. */
static int scenario_fault(const char *kind, int catch)
{
    Module good, bad, after;
    unsigned code = strcmp(kind, "null") == 0 ? 101 : strcmp(kind, "stack") == 0 ? 102 : 103;

    desk_register_exports();
    mod_catch_faults(catch);
    say("host: fault %s, signal handlers %s", kind, catch ? "on" : "off");
    if (open_module(&good, "HELLO", 1, 'a', "good") != 0 || open_module(&bad, "CRASH", 2, 'a', "") != 0)
        return 1;
    send(&good, 1, 1, 1);
    send(&bad, code, 0, 0);
    say("host: survived the fault, state %s", bad.state == MOD_CRASHED ? "crashed" : "not crashed");
    mod_unload(&bad);
    send(&good, 2, 2, 2);
    if (open_module(&after, "COUNTER", 3, 'a', "") != 0)
        return 1;
    send(&after, 1, 5, 0);
    mod_stop(&after);
    mod_unload(&after);
    mod_stop(&good);
    mod_unload(&good);
    say("host: fault %s recovered, the core kept running", kind);
    return 0;
}

/* The export table is not registered: dlopen must refuse the module and say which import it could not resolve, and must leave
 * the core able to load it once the table is there. */
static int scenario_noreg(void)
{
    Module m;

    if (mod_load(&m, "HELLO", "HELLO.DXE", 1, 'a') == 0) {
        say("host: loaded without the export table (unexpected)");
        return 1;
    }
    desk_register_exports();
    if (open_module(&m, "HELLO", 1, 'a', "late") != 0)
        return 1;
    mod_stop(&m);
    mod_unload(&m);
    say("host: noreg ok, the failed dlopen left nothing behind");
    return 0;
}

/* Load, run and unload the same module many times and compare memory before and after. */
static int scenario_leak(int rounds, int crashing, int control)
{
    unsigned long v0 = 0, h0 = 0, v1, h1, vm, hm;
    int i;
    Module m;

    desk_register_exports();
    mod_catch_faults(1);
    for (i = 1; i <= rounds; i++) {
        desk_quiet = !(i <= 2 || i == rounds);
        if (crashing) {
            if (open_module(&m, "CRASH", 1, 'a', "") != 0)
                return 1;
            send(&m, 104, 0, 0);
        } else {
            if (open_module(&m, "HELLO", 1, 'a', "x") != 0)
                return 1;
            send(&m, 1, i, i);
            send(&m, 2, i, i);
            mod_stop(&m);
        }
        mod_unload(&m);
        if (control)
            desk_alloc(4096); /* a deliberate leak (no owner, so never reclaimed): the measure must notice it */
        if (i == 1)
            mem("after 1", &v0, &h0);
        if (i % 50 == 0)
            mem("round", &vm, &hm);
    }
    desk_quiet = 0;
    mem("after all", &v1, &h1);
    say("leak: %d rounds of load, run%s, unload%s", rounds, crashing ? ", panic" : ", stop", control ? " (with a deliberate 4 KB leak per round)" : "");
    say("leak: virtual memory after round 1 versus after round %d: %s", rounds, v1 >= v0 ? "not lower (no growth)" : "LOWER (leak)");
    say("leak: heap break after round 1 versus after round %d: %s", rounds, h1 <= h0 ? "not higher (stable)" : "HIGHER (leak)");
    return v1 >= v0 && h1 <= h0 ? 0 : 1;
}

int main(int argc, char **argv)
{
    const char *s = argc > 1 ? argv[1] : "";

    remove("OUT.TXT");
    if (strcmp(s, "life") == 0)
        return scenario_life('a');
    if (strcmp(s, "life-b") == 0)
        return scenario_life('b');
    if (strcmp(s, "two") == 0) {
        mod_global = argc > 2 && strcmp(argv[2], "global") == 0;
        return scenario_two('a');
    }
    if (strcmp(s, "two-b") == 0)
        return scenario_two('b');
    if (strcmp(s, "panic") == 0)
        return scenario_panic('a');
    if (strcmp(s, "panic-b") == 0)
        return scenario_panic('b');
    if (strcmp(s, "fault") == 0 && argc > 3)
        return scenario_fault(argv[2], strcmp(argv[3], "catch") == 0);
    if (strcmp(s, "noreg") == 0)
        return scenario_noreg();
    if (strcmp(s, "leak") == 0)
        return scenario_leak(argc > 2 ? atoi(argv[2]) : 200, 0, 0);
    if (strcmp(s, "leak-crash") == 0)
        return scenario_leak(argc > 2 ? atoi(argv[2]) : 200, 1, 0);
    if (strcmp(s, "leak-control") == 0)
        return scenario_leak(argc > 2 ? atoi(argv[2]) : 200, 0, 1);
    say("usage: HOST.EXE life|life-b|two|two-b|panic|panic-b|fault <null|stack|div> <catch|raw>|two global|noreg|leak [n]|leak-crash [n]|leak-control [n]");
    return 2;
}
