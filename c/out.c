/* Result output: one line to stdout and to OUT.TXT.
 *
 * (c) Copyright 2026 Liminal HQ, Scott Morris
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 *
 * The file copy exists because capturing a DOS program's stdout under a headless DOSBox is fiddly (redirection through the DOS
 * shell, line ends, a crash dump on stderr). The test harness reads OUT.TXT instead and does not depend on redirection.
 */
#include <stdio.h>

#include "out.h"

void desk_emit(const char *line)
{
    FILE *f;

    puts(line);
    fflush(stdout);

    f = fopen("OUT.TXT", "a");
    if (f != NULL) {
        fputs(line, f);
        fputc('\n', f);
        fclose(f);
    }
}
