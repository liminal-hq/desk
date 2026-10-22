# spikes

Small experiments that decide the architecture, each in its own folder and each ending with a written result in `docs/` (what worked, what did not, what it changes):

1. **s1-dxe3**: can Rust code run as a DXE3 module that calls back into the core?
2. **s2-vbe**: VBE 2.0 linear frame buffer and VGA mode 12h from Rust.
3. **s3-mouse**: INT 33h from Rust.
4. **s4-handoff**: `DESK.COM` hands the machine to a real-mode program and takes it back.
5. **s5-jsdos**: does the browser build work with the linear frame buffer?
6. **s6-ttf**: can an integer rasteriser draw a real TrueType face at 14 px and 24 px on a 486 within the frame budget, and how does it look beside the bitmap faces? Later than the others; see `docs/OPEN-QUESTIONS.md`, "Reading TrueType fonts".

Spike code is experimental and may be thrown away; it is labelled `spike` and `experimental` on GitHub.
