# The present API and the framebuffer lifetime

A decision note for the owner on roadmap R136. It changes no code. Every claim about code cites a file and line read on `main` at `700d2eb`; an **inference** is marked. Fizz 2 paths are in the checkout at `/home/scott/source/experiments/fizz-2`, read and not copied.

## The problem

`Display::set_mode` hands the caller an owned `FrameBuffer` whose `region: MmioRegion` is a raw pointer and a length (`hal/hal/src/video.rs:94-101`, `:106`; `hal/hal/src/mmio.rs:13-16`). Nothing in the type ties it to the display or to the mode. `MmioRegion::new` is `unsafe` and its contract is that the memory stays mapped "for as long as the region lives" (`mmio.rs:22-28`), but every method on the region is safe and checks only the length (`mmio.rs:38-43`).

**Hosted track.** `DosDisplay` keeps one live VBE mapping in a global (`dos/rt/src/hal.rs:100`). A second VBE `set_mode` unmaps the first (`hal.rs:237`) and `restore_text_mode` unmaps it too (`hal.rs:103-115`, `:259-261`), while the earlier `FrameBuffer` still holds the old pointer and its safe `write8` still passes the bounds check (`hal.rs:246-249` made it; the `SAFETY` comment says "valid until `restore` unmaps it"). The mode 13h and 12h windows are never unmapped (`hal.rs:189-199`), so the hole is VBE only, but the comment there makes the same promise: valid "for as long as a graphics mode is set". `DosDisplay::new` is public and the mapping is a static, so two displays share it; `spikes/a-vga-vbe/prog/src/main.rs:395` and `:414` construct more than one.

**Native track.** The kernel never implements `Display`. It builds regions with two safe functions that each mint an `MmioRegion` (`kernel/src/gfx.rs:36-46`) and calls them freely, including twice over the same memory for the corrupt-pixel test (`kernel/src/shell.rs:249-251`, `:266-269`). Modes are set by registers (`shell.rs:223`, `gfx.rs:596`, `:614`) or by the loader. The same gap is present as "any code can make a region", not as a stale one. This is why the gap is recorded as open in `docs/RUST-DOS-INTEGRATION-ANALYSIS.md:61-67`, "needs resolution before exposing mode switching to apps".

## What each consumer needs

- **The shell's presenters.** `desk_gfx::Present` is a span writer: `begin`, `span(x, y, &[u8])`, `end`, and an `align` of 1 or 8 (`shell/gfx/src/present.rs:11-28`). `LinearPresent` is one `write_bytes` per span at `y * pitch + x` (`shell/present/src/linear.rs:43-48`). `PlanarPresent` turns an 8-aligned span into four plane writes, selecting each plane through the sequencer map mask and resetting the graphics controller once per frame (`planar.rs:65-93`, `:153-187`). Both hold `&mut MmioRegion` (`linear.rs:11`, `planar.rs:52`) and are built per frame from `Screen.fb.region` (`shell/dos/src/main.rs:160`, `:165-181`). The read-back used by the checks needs the region too (`main.rs:189-190`).
- **The kernel.** The same presenters over a region it makes itself (`kernel/src/shell.rs:216-218`, `:225-228`). It needs a mode set by registers or the loader, with no DPMI mapping to release.
- **A future app surface (R79).** Apps never see a `FrameBuffer` (`docs/HAL.md:408`). They `desk_blit` palette indices into a window (`docs/SDK.md:44-57`) or draw into a shared client buffer presented "with a damage rectangle" (`docs/APP-MODEL.md:176`, `:195-197`). So an app's pixels reach the screen only through the compositor's span presents. **Inference:** the risk R79 adds is not an app holding a region but the system code that switches modes for a DOS hand-off or an app asking for one, which is the "mode switching" the analysis warns about.
- **Fizz 2.** One whole 320 by 200 frame per tick. Its trait is `present(&mut self, pixels: &[u8], pitch: usize)` and `set_mode` returns only a `ModeInfo` (`crates/fizz-hal/src/video.rs:42`, `:50`). The DOS implementation refuses any pitch but 320 and copies 64,000 bytes to A0000h with `copy_nonoverlapping` (`dos/fizz2/src/hal.rs:101-108`; `dos/rt/src/vga.rs:92-96` there). It has no planar or VBE path and no damage.

## The three options

### A. Whole-frame `present`, owned by the display

```rust
fn set_mode(&mut self, id: u16) -> Result<ModeInfo, HalError>;
fn present(&mut self, pixels: &[u8], pitch: usize) -> Result<(), HalError>;
```

`FrameBuffer` and the public `MmioRegion` surface leave `Display`. **hal:** the trait changes and `FrameBuffer` goes. **dos/rt:** `present` does the linear copy, the plane split and the plane selection that `shell/present` does now; the mapping stays private. **shell/present:** the presenters stop owning the pixel step. Planarising (`planar.rs:16-31`) must move down into `hal/drivers`, because the HAL cannot depend on `shell/` (`docs/LAYERS.md`; **inference** that `tools/layers` would refuse the other way). **Kernel:** gains a `Display` implementation it does not have today. **R79:** unchanged, since apps go through the compositor. **Stays:** the back buffer, damage tracking and the DAC calls.

- **Host tests.** Easy: a memory display that records the frame, as Fizz 2's mock does (`crates/fizz-hal/src/mock.rs:193`). The repository has no mock `Display` today (`hal/platform/mock/src` has a BIOS, bus, IRQ and VGA mocks only), so any option adds one.
- **Performance.** A whole 640 by 480 frame is 307,200 source bytes per present at depth 8 (arithmetic), against only the damaged spans now. Whether that matters on a 486 is **not measured here**. Fizz 2's figure of 3,589 to 599 µs is for a string-instruction copy, in counted instructions in DOSBox, and was not reproduced (`docs/ROADMAP.md`, R132); it says nothing about this API shape.
- **Safety.** Statically: no region escapes, so the gap is closed by construction. Only at run time: pitch and length, as in Fizz 2's check. A volatile-contract question reopens, since Fizz 2 copies non-volatilely and Desk's region is volatile (`mmio.rs:11`; R132).
- **Migration.** Five steps: trait, DOS implementation, planarise down, presenters retired or thinned, kernel display. Large (**inference**: several hundred lines and every presenter test).

### B. A mode session that owns the mapping

```rust
trait Display { type Session<'d>: ModeSession where Self: 'd;
    fn set_mode(&mut self, id: u16) -> Result<Self::Session<'_>, HalError>; /* modes, palette: as now */ }
trait ModeSession { fn info(&self) -> ModeInfo; fn align(&self) -> u32;
    fn begin(&mut self) -> Result<(), HalError>;
    fn present_span(&mut self, x: u32, y: u32, pixels: &[u8]) -> Result<(), HalError>;
    fn end(&mut self) -> Result<(), HalError>;
    fn read_back(&mut self, out: &mut [u8]) -> Result<(), HalError>; }
```

Dropping the session restores text mode and releases the mapping. Planes are chosen inside the session, as `PlanarPresent` does now; exposing `select_plane` as well is possible but leaks mode 12h into the trait. The argument order is `(x, y)` as in `Present::span` (`present.rs:23`), not the `(y, x)` of R136, to avoid a transposition bug.

**hal:** `ModeSession` is `Present` moved down (**inference**: the HAL cannot name `desk_gfx`, so the trait lives in `hal` and `desk_gfx` re-exports it). **dos/rt:** a session type holding the display's exclusive borrow, the mapping and the plane logic; `Drop` replaces the static (`hal.rs:100`) and the clean-up entry stays as the panic path. **shell/present:** the two presenters become the session's internals or generic over a smaller sink (`PlaneSink` already is one, `planar.rs:34-47`); `Screen` (`main.rs:158-161`) holds a session. **Kernel:** a session over its own region. **R79:** the compositor owns the one session; a hand-off drops it. **Stays:** damage, spans, alignment, and `PlaneSink` tests.

- **Host tests.** A memory session in the mock records spans, like `MemoryPresent` (`present.rs:42`). `PlanarPresent::with_sink` is already tested without hardware (`planar.rs:121`).
- **Performance.** Same bytes as today: damage spans stay. One extra indirect call per span (a `dyn` is already used, `main.rs:165`). No measurement exists here.
- **Safety.** Statically: no region leaves the session, and the session cannot outlive the display borrow or a second `set_mode`. At run time: bounds and alignment. Not closed: two `DosDisplay` values sharing the static still need a one-instance guard (**inference**; the same holds for A and C).
- **Migration.** Six steps; medium-large (**inference**).

### C. Borrow the region from the display

```rust
fn set_mode(&mut self, id: u16) -> Result<FrameBuffer<'_>, HalError>;
pub struct FrameBuffer<'d> { /* info, pitch, phys, region private */ _d: PhantomData<&'d mut ()> }
impl FrameBuffer<'_> { pub fn region(&mut self) -> &mut MmioRegion; }
```

Smallest change: `set_mode`, `restore_text_mode` and the next `set_mode` all take `&mut self`, so the borrow checker refuses them while a `FrameBuffer` lives. **Closes:** a retained region surviving a restore or a second mode set. **Does not close:** a region moved out of a public field (`pub region`, `video.rs:97`) would carry no lifetime, so the fields must go private as above; the unmap-on-panic path (`hal.rs:111`), which runs outside the borrow; two displays; and the kernel's freely minted regions. **Breaks as written:** `start_screen` calls `display.set_palette` while `fb` is alive (`main.rs:218`, `:226`) and `scenario`, `mark` and `interactive` take `&DosDisplay` beside a `Screen` (`main.rs:256`, `:332`, `:370`). **Inference** from reading, not compiling: palette and retrace would move onto the handle or become `&self`.

**hal, dos/rt:** a signature and visibility change plus the palette move. **shell/present, kernel, R79:** unchanged, because presenters still take a region. **Host tests:** unchanged (Vec-backed regions, `shell/present/tests/linear.rs:10-12`). **Performance:** none, the same code. **Safety:** compile time for the borrow; the `unsafe` constructors are still reachable by any caller. **Migration:** two steps, small.

## Comparison

| | A. Whole frame | B. Session | C. Borrow |
|---|---|---|---|
| Closes the gap | By construction | Yes, and the mapping is released on drop | Partly: borrow only |
| Region visible to safe code | No | No | Yes, by `&mut` |
| Damage spans and mode 12h | Move into the HAL, or lose them | Kept | Kept |
| Kernel | New `Display` | New session | Unchanged |
| R79 | Unchanged | Compositor owns the session | Compositor owns the borrow |
| Host test | Memory display | Memory session | Existing |
| Bytes per present | Whole frame (**not measured**) | Damage spans | Damage spans |
| Size (inference) | Large | Medium-large | Small |

## Recommendation

**B, reached in two steps.** First do C with private fields (one pull request, no presenter change), because it already makes mode changes safe in the single-screen case and exposes the `set_palette` conflict early. Then make the borrowed handle a session that owns the mapping and presents spans, which removes `MmioRegion` from the public display surface and gives R79 one owner to drop on a hand-off. A is rejected for Desk: it gives up damage spans or moves the whole presenter into the HAL, and its speed advantage is unmeasured; take its idea (the display holds the pointer), not its signature.

Two questions only the owner can answer:

1. Should mode restore be tied to dropping the session (**yes** or **no**), or stay the explicit `restore_text_mode` call?
2. May `MmioRegion` leave the public `Display` surface entirely (**yes** or **no**), or must the shell keep reaching a region for read-back and the kernel's checks?

## What could not be verified

- Nothing was compiled or run: no borrow-check result, no size, no timing. The three signatures are sketches, and the sizes and the C conflicts are **inferences** from reading.
- Whether `tools/layers` accepts `Present` moving into `hal`.
- Whether a `rep movsd` copy meets the volatile contract (R132).
- Fizz 2's performance claims; I read how its API is shaped and did not run it.
- How an app asks for a different mode, since no part of R79 is built; the R79 effect above is from the design documents only.
