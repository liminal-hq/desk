/* The Desk's entry point: calls into the Rust core and reports what it returned.
 *
 * (c) Copyright 2026 Liminal HQ, Scott Morris
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 *
 * usage: DESK.EXE [panic]
 *
 * With no argument the program prints the Rust core's result line and exits 0. With `panic` it asks the core to panic on purpose,
 * which must report through desk_panic() and exit 3: the checks use it to prove the failure path.
 */
#include <stdio.h>
#include <string.h>

#include "out.h"

/* The Rust core's exports (core/src/lib.rs). */
extern int desk_main(char *out, size_t capacity);
extern void desk_selftest_panic(void);

int main(int argc, char **argv)
{
    char line[128];
    int length;

    /* A stale OUT.TXT from an earlier run must not be mistaken for this run's result. */
    remove("OUT.TXT");

    if (argc > 1 && strcmp(argv[1], "panic") == 0) {
        desk_selftest_panic();
        return 1; /* not reached: the panic handler exits */
    }

    length = desk_main(line, sizeof line);
    if (length < 0) {
        desk_emit("DESK-SKELETON failed: the result did not fit its buffer");
        return 2;
    }
    desk_emit(line);
    return 0;
}
