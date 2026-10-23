// Port I/O: the one surface every legacy PC device sits behind.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The unused POST diagnostic port. A write to it takes about a microsecond on the ISA bus and has no other effect, which makes it
/// the traditional delay between accesses to slow devices such as the 8259.
pub const POST_PORT: u16 = 0x80;

/// Port-mapped I/O, 8, 16 and 32 bits wide.
///
/// The methods are safe to call: a value of a type that implements `PortIo` is the capability, and creating one for real hardware
/// is the `unsafe` step (see `x86::RawPorts::new`). They take `&self` so several drivers can share one bus; on hardware the bus is a
/// zero-sized `Copy` value, and the mock uses interior mutability.
///
/// Ordering: every access is performed, in program order, exactly once. The real implementation is an `in` or `out` instruction,
/// which the compiler may not remove, merge or reorder with other port accesses (each is an `asm!` block, which is a compiler
/// barrier for other `asm!` blocks); x86 port I/O is also strongly ordered by the CPU. Port accesses are not ordered against plain
/// memory accesses: a driver that hands a buffer to a device (DMA) and then starts it with a port write needs a compiler fence
/// (`core::sync::atomic::compiler_fence(Ordering::SeqCst)`) between the two. The x86 implementation omits `nomem` on its `asm!`
/// blocks, so they already act as such a fence; a platform that cannot guarantee it must document so.
pub trait PortIo {
    fn inb(&self, port: u16) -> u8;
    fn outb(&self, port: u16, value: u8);
    fn inw(&self, port: u16) -> u16;
    fn outw(&self, port: u16, value: u16);
    fn inl(&self, port: u16) -> u32;
    fn outl(&self, port: u16, value: u32);

    /// A short delay for slow devices: a write to the POST port. On a 486 at ISA speed this is about 1 µs. It is a delay by bus
    /// cycle, not by time, so code that needs a measured wait uses `time::Delay` instead.
    fn io_wait(&self) {
        self.outb(POST_PORT, 0);
    }
}

/// A shared reference to a bus is a bus, so a driver can hold `&P` while another holds the same bus.
impl<P: PortIo + ?Sized> PortIo for &P {
    fn inb(&self, port: u16) -> u8 {
        (**self).inb(port)
    }
    fn outb(&self, port: u16, value: u8) {
        (**self).outb(port, value)
    }
    fn inw(&self, port: u16) -> u16 {
        (**self).inw(port)
    }
    fn outw(&self, port: u16, value: u16) {
        (**self).outw(port, value)
    }
    fn inl(&self, port: u16) -> u32 {
        (**self).inl(port)
    }
    fn outl(&self, port: u16, value: u32) {
        (**self).outl(port, value)
    }
    fn io_wait(&self) {
        (**self).io_wait()
    }
}
