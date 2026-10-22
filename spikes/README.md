# spikes

Small experiments that decide the architecture, each in its own folder and each ending with a written result in `docs/` (what worked, what did not, what it changes):

0. **d0-rust-dos**: can a DOS program be pure Rust, with no C layer and no DJGPP libc? Done: yes (`docs/D0-RUST-DOS.md`).
1. **s1-dxe3**: can Rust code run as a DXE3 module that calls back into the core?
2. **s2-vbe**: VBE 2.0 linear frame buffer and VGA mode 12h from Rust.
3. **s3-mouse**: INT 33h from Rust.
4. **s4-handoff**: `DESK.COM` hands the machine to a real-mode program and takes it back.
5. **s5-jsdos**: does the browser build work with the linear frame buffer?

The walking skeleton that proves the build toolchain is done (`docs/TOOLCHAIN.md`); the next spike is `s1-dxe3`.

Spike code is experimental and may be thrown away; it is labelled `spike` and `experimental` on GitHub.
