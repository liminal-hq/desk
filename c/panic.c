/* The Rust panic handler's way out: report through C and exit with a failure status.
 *
 * (c) Copyright 2026 Liminal HQ, Scott Morris
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 *
 * Later this is where the screen goes back to text mode and every hooked interrupt vector is restored before the program exits,
 * so a panic leaves the machine as it found it. Nothing is hooked yet.
 */
#include <stdio.h>
#include <stdlib.h>

#include "out.h"

/* Called from core/src/lib.rs. Only an integer crosses the boundary: the source line of the panic. */
void desk_panic(unsigned line)
{
    char text[64];

    snprintf(text, sizeof text, "DESK-SKELETON PANIC at line %u", line);
    desk_emit(text);
    exit(3);
}
