/* Result output: one line to stdout and to OUT.TXT.
 *
 * (c) Copyright 2026 Liminal HQ, Scott Morris
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */
#ifndef DESK_OUT_H
#define DESK_OUT_H

/* Prints `line` and a newline to stdout, and appends the same line to OUT.TXT in the current directory. Both are flushed before
 * returning, so the text survives a crash or an exit straight afterwards. */
void desk_emit(const char *line);

#endif
