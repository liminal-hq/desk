// Hardware proof: an IRQ 0 hook counting ticks for about a second, and VGA mode 13h written and read back.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![no_main]

rt::entry!(progs::hw::main);
