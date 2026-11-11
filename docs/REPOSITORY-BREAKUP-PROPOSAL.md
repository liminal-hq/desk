# Repository breakup: three ways to separate Desk from its platform

Proposal date: 2026-10-24. Status: suggested structures and migration slips, not an approved extraction or a change to [Direction](DIRECTION.md). Repository names below are working names under `liminal-hq`; their availability has not been checked. No repositories have been created or content moved.

The aim is to make **Desk the shell** a clear product above reusable Rust/DOS machinery and a native operating-system foundation. The complete bootable product can still be called Desk; its release assembly need not live beside its desktop implementation. These are ownership and release boundaries, independent of future decisions about processes, protection or module formats.

## Current boundaries and constraints

The checkout already has useful crate boundaries: [shared HAL and drivers](../hal/Cargo.toml), [shared shell](../shell/Cargo.toml), [DOS executable wiring](../shell/dos/Cargo.toml), [DOS runtime](../spikes/d0-rust-dos/rt/Cargo.toml) and [native kernel](../kernel/Cargo.toml). They use relative path dependencies. Moving directories alone would break those paths, shared target references, font inclusions, linker-script references and the scripts that compare both platforms.

The [integration analysis](RUST-DOS-INTEGRATION-ANALYSIS.md) identifies contracts to settle before treating the components as independent products: resource restoration, framebuffer validity, module recovery and overlapping SDK and experimental `std` symbols. The DOS runtime already consumes shared drivers; the kernel directly consumes shell crates. An independent kernel needs a deliberate change to where its shell entry is composed.

Repository separation does not itself produce runtime isolation, a stable Rust ABI or a complete SDK. It adds version coordination and independent builds. Extract where that cost buys another useful consumer, a distinct maintenance schedule or a clearer responsibility.

## The three breakdowns at a glance

| | Option 1: minimal extraction | Option 2: four practical boundaries | Option 3: full layer cake |
|---|---|---|---|
| Repositories, including `desk` | 2 | 4 | 8 |
| Main split | Desktop/product versus reusable platform kit | Shell, shared hardware, DOS runtime, native system/release assembly | Contracts, drivers, runtimes, kernel, app ABI, shell, apps and distribution |
| What `desk` owns | Shell, apps, kernel and complete product integration | Shell, SDK, apps, fonts and shell-specific adapters | Shell, desktop UI, fonts and shell-specific adapters |
| Shared hardware ownership | Bundled in the platform kit | Dedicated HAL/driver workspace | Separate contract and driver repositories |
| Full-system integration | `desk` | `desk-system` | `desk-system` |
| Best fit | First extraction while contracts still change together | Reusable DOS work and a separately evolving kernel | Mature components with several consumers and independent release needs |
| Principal cost | Platform kit groups different lifecycles | Kernel and release assembly still share a repository | Coordinated updates and many compatibility combinations |

**Recommendation:** start physically with Option 1 while preserving crate boundaries that permit Option 2. Option 2 is the suggested medium-term structure. Option 3 becomes worthwhile when its separate layers have working consumers and maintenance needs.

## Option 1: extract one reusable platform kit

Create `liminal-hq/pc-platform`. Keep the shell and complete product in `liminal-hq/desk`.

| Repository | Responsibility | Suggested contents |
|---|---|---|
| `desk` | Desktop and product integration on both tracks | `shell/`, fonts, SDK and apps as they arrive, `kernel/`, `boot/`, hosted launcher, product specifications, golden frames, cross-track comparison and release assembly |
| `pc-platform` | Reusable 486 platform machinery with a DOS runtime | Entire `hal/` workspace, promoted DOS `rt`, DOS ELF target and linker script, ELF-to-go32 packer, standalone DOS examples, runtime/driver tests and relevant tool setup |

Suggested new repository layout:

```text
pc-platform/
  hal/                 # HAL traits, shared drivers and mock workspace
  dos/rt/              # Promoted runtime, including the DOS platform adapter
  dos/targets/         # DOS target specification and linker script
  tools/               # Packer and standalone DOS build/run support
  examples/            # Console, allocation, files and hardware probes
  docs/                # Runtime contracts, assumptions and spike results
```

Dependency direction:

```text
desk hosted adapter    -> pc-platform DOS runtime + shared hardware
desk native kernel    -> pc-platform shared hardware
standalone DOS program -> pc-platform DOS runtime + shared hardware
```

The platform kit has no dependency on the shell, its widgets, its fonts or its test scenes. A standalone DOS program is an independent consumer. The native kernel consumes only the hardware packages, even though they live beside DOS-specific packages.

**What moves first:** promote `rt` inside the checkout, settle its public lifecycle contract, then extract it together with the HAL workspace it already consumes. Move the target, linker and packing assets with it so the new repository can build an executable independently. Keep the C/DJGPP skeleton and DXE3 host in Desk until their replacements are demonstrated.

**Why this works:** only one new dependency boundary is introduced. Platform changes can still update HAL, drivers and runtime together. Standalone Rust/DOS work gets a home without making every layer a separate maintenance obligation.

**Tradeoff:** this is a convenience boundary. A native consumer depends on a repository that carries DOS tools, though its hardware-crate tests should not require those tools. Keep native and DOS build assets separate internally. If DOS consumers need an independent release schedule, move towards Option 2.

**Exit gate:** clean checkouts build without unversioned sibling-directory assumptions; Desk's hosted/native frames still match; the platform repository passes standalone DOS and host-driver checks. The extracted revision and consumer update must be reviewable before switching the authoritative source.

## Option 2: four practical repositories

Separate shared hardware from the DOS runtime, and place the native kernel with system assembly. Desk the shell gets a clearer identity while the number of release boundaries remains manageable.

| Repository | Owns | Depends on |
|---|---|---|
| `pc-hal` | HAL contracts, shared drivers, mock platform, host tests and portable hardware specifications | Rust `core`/`alloc` as needed; no Desk or runtime dependency |
| `dos-rs` | DOS runtime/platform adapter, DPMI/DOS services, executable target/linker/packer assets, standalone examples and experimental DOS `std` work | `pc-hal` |
| `desk` | Shared shell, compositor, presenters, UI, window manager, fonts, app SDK/apps, hosted and native shell adapters, hosted launcher when implemented | `pc-hal`; hosted adapter additionally consumes `dos-rs` |
| `desk-system` | Native kernel, boot code, native platform implementation, native emulator checks, pinned dependencies and complete DOS/native product assembly | `pc-hal`, `desk`, and `dos-rs` for hosted assembly |

Suggested internal layouts for the two product repositories:

```text
desk/                         desk-system/
  shell/                        kernel/
  adapters/dos/                 boot/
  adapters/native/              compose/        # Final native executable
  launcher/                     release/        # DOS/native assembly recipes
  sdk/                          integration/    # Both-track checks and pins
  apps/
  fonts/
```

The foundation dependencies are `dos-rs -> pc-hal` and the native kernel's crates `-> pc-hal`. The shell consumes HAL contracts. The system build composes the shell with either foundation. `desk-system` owns the native implementation and the release recipe in this option; its kernel crates do not depend on the release assembler.

**What changes relative to Option 1:** split the kit into `pc-hal` and `dos-rs`; move native boot/kernel ownership and whole-product assembly out of Desk. Keep traits, drivers and mocks together because their contracts still evolve together. Keep shell and reference apps together while a usable app API develops.

**Native composition seam:** move shell-specific startup out of `kernel/src/shell.rs` into a Desk-owned adapter once the kernel can supply the services it needs. A thin final executable in `desk-system` wires that adapter to kernel services. Kernel-only tests remain free of Desk's fonts and golden scenario. This is a proposed refactor: the current kernel is an executable, not an established reusable kernel library.

**Hosted composition seam:** Desk owns the executable that translates DOS input into shell events, chooses UI configuration and supplies assets. `dos-rs` owns generic `DosDisplay`, IRQ and file services. The launcher belongs to Desk because `DESK.RUN` and resume policy are product behaviour. `desk-system` packages its output into a tested hosted release.

**Why this is the preferred medium-term option:** a DOS game can consume `dos-rs`, another native project can consume `pc-hal`, and Desk can develop its desktop and SDK without owning either foundation. The system repository supplies one tested combination for contributors and users.

**Tradeoff:** `desk-system` combines native kernel development and product distribution. That is acceptable while one kernel and shell evolve together. Option 3 separates them when the kernel has independent consumers or releases.

**Exit gate:** `pc-hal` tests without DOS tooling, `dos-rs` builds a standalone executable without Desk, Desk runs host shell tests without a kernel, and `desk-system` produces both variants from pinned revisions. A release records the dependency combination that passed cross-track comparison.

## Option 3: full layer cake

Use eight repositories once the boundaries justify independent maintenance. Each can contain several Rust crates; this is not one repository per crate.

| Layer and proposed repository | Responsibility | Source or future work from this checkout |
|---|---|---|
| 1. `pc-hal` | Platform-neutral service contracts, memory/device access types, common events and mock facilities | `hal/hal/` and `hal/platform/mock/`; rearrange mock models only where needed to prevent driver dependency cycles |
| 2. `pc-drivers` | Shared device algorithms, register programming and driver conformance tests | `hal/drivers/`; keep DOS-specific drivers feature-separated for applicable consumers |
| 3a. `dos-rs` | Hosted DOS foundation and executable build assets | Promoted D0 runtime, DOS HAL implementation, DPMI/DOS calls, target, linker, packer and standalone tests |
| 3b. `desk-kernel` | Native foundation, independent boot path, native HAL and kernel-only emulator checks | `kernel/`, native `boot/` code and kernel build/image helpers after shell composition is separated |
| 4. `desk-api` | Versioned app ABI, C header, Rust SDK, app-side allocator/panic bridge and compatibility fixtures | Planned `sdk/` implementation informed by the DXE3 and `std` experiments; no shell or kernel implementation |
| 5. `desk` | Desktop product behaviour, app-host implementation and platform adapters | Shared shell crates, fonts, UI assets, themes, app host, hosted launcher and DOS/native adapters |
| 6. `desk-apps` | First-party apps, manifests, packages and app compatibility tests | New app workspace once apps exist; module examples remain experiments until promoted |
| 7. `desk-system` | Distribution and compatibility assembly | Exact dependency revisions, package selection, bootable images, DOS bundles, installation layout and end-to-end checks |

The two foundations are parallel, so the structure branches below the desktop:

```text
Applications:       desk-apps -> desk-api
Desktop:            desk -> desk-api + pc-hal
DOS adapter:        desk DOS adapter -> dos-rs
Native composition: final executable -> desk native adapter + desk-kernel
Foundations:        dos-rs / desk-kernel -> pc-drivers + pc-hal
Hardware:           pc-drivers -> pc-hal
Distribution:       desk-system pins and builds the tested combination
```

**Keep the app contract independent:** `desk-api` defines the app interface; Desk supplies the host implementation. Apps depend on the SDK, not on shell internals. A host must be able to consume event types and version constants without linking an app-side global allocator or panic handler; use separate crates or explicit features for those pieces.

**Avoid the kernel/shell cycle:** a final native executable in `desk-system` depends on both `desk-kernel` and Desk's native adapter. The kernel exports the platform services needed by that composition and retains a shell-free boot/test executable. Establish the seam in working code before extraction; it does not require a generic plugin framework.

**Place module loading by responsibility:** a foundation owns format mechanics, executable memory and platform fault hooks; Desk owns app identity, SDK binding, windows and lifecycle policy; `desk-api` owns the app-visible contract. A loader must not import window-manager internals. Keep a portable loader as a crate with its current owner until another consumer warrants moving it.

**Place `std` by maturity:** retain the current DOS patch/shim in `dos-rs`. Move a common app-facing platform contract into `desk-api` only when hosted and native implementations exist and signature conflicts are resolved. A dedicated `rust-std-port` repository is a possible later project if toolchain maintenance warrants it; it is outside this eight-repository breakdown.

**Keep fonts with their current product:** font sources, build scripts, assets and licences stay in Desk. Apps use SDK text services or explicit packaged assets. Extract fonts only when another consumer needs independent releases.

**Why this can work:** each component has a defined audience, acceptance suite and release responsibility. DOS programming can advance without desktop releases, the kernel can be checked without UI changes, and apps can declare compatibility against a small public SDK.

**Tradeoff:** an event-type change can require coordinated updates to contracts, drivers, foundations, SDK, shell, apps and distribution pins. Repository count magnifies work while interfaces remain volatile. Source separation also leaves trusted modules in the same address space unless runtime architecture changes.

**Exit gate:** each component has independently useful tests, an identified maintenance responsibility and a real consumer. System assembly reproduces the exact tested combination, including assets and target tooling. App compatibility failures are detected before packaging.

## Mapping the current checkout to future ownership

| Current area | Option 1 | Option 2 | Option 3 |
|---|---|---|---|
| Shared `shell/` crates | `desk` | `desk` | `desk` |
| `shell/dos` | `desk` | `desk` | Desk adapter; distribution packages it |
| `hal/hal` and mock platform | `pc-platform` | `pc-hal` | `pc-hal` |
| `hal/drivers` | `pc-platform` | `pc-hal` | `pc-drivers` |
| D0 runtime, DOS ELF target/linker, `elf2go32.py` | `pc-platform` | `dos-rs` | `dos-rs` |
| Native kernel and boot implementation | `desk` | `desk-system` | `desk-kernel` |
| Native shell wiring currently in kernel | `desk` | Desk adapter composed by `desk-system` | Desk adapter composed by `desk-system` |
| C/DJGPP skeleton, COFF converter, DXE3 spike | `desk` while needed | `desk` while needed | Promote generic loader mechanics into a foundation only when ready; retain historical experiments in Desk |
| SDK and first-party apps | `desk` | `desk` | `desk-api` and `desk-apps`, once implemented |
| Experimental `std` patch/shim | `pc-platform` | `dos-rs` | `dos-rs` initially; shared ABI moves after validation |
| Fonts and desktop assets | `desk` | `desk` | `desk` |
| Hosted launcher and session policy | `desk` | `desk` | `desk` |
| Complete-product checks and release assembly | `desk` | `desk-system` | `desk-system` |

Move spikes with the responsibility they validate when their imports and purpose remain useful. Split mixed probes deliberately: standalone runtime exercises belong with a foundation, while shell scenes and golden frames belong with Desk. A low-level repository should not depend on Desk simply to preserve an old experiment unchanged.

## Dependency, release and test rules for every option

**Keep one authoritative implementation.** Prepare and test the extracted package before switching consumers. Once switched, remove or clearly archive the former implementation. Avoid copied active runtime/driver trees. Historical reports can remain with a relocation note and stable reference.

**Pin the entire build.** Rust dependencies can use exact Git revisions initially, with committed lockfiles for executable builds. Target JSON, linker scripts, packers and stubs also need explicit acquisition. Cargo dependencies alone do not replace shell-script references to sibling files. Provide a pinned source checkout or versioned build-assets bundle with checksums rather than requiring unversioned sibling repositories.

**Support coordinated local work.** A documented workspace overlay or Cargo patch configuration can substitute local checkouts. Production builds use pinned inputs. An integration job can test proposed revisions together before distribution pins change.

**Preserve the target matrix.** Shared libraries retain host tests and both custom-target builds where applicable. DOS checks stay with the DOS foundation, kernel boot checks with its owner, and full-system frame comparison with product integration. Keep authoritative shell scenarios/goldens in Desk; supply versioned test artefacts to integration instead of duplicating mutable golden sets.

**Keep tool provisioning distinct.** Continue using the existing shared CI image infrastructure for Rust and emulator tools. Each repository declares its required image and assets. It does not need a duplicate toolchain project. Toolchain bumps test affected targets and any maintained `std` patch.

**Preserve history and attribution.** Choose a history-preserving extraction procedure when executing a split, retain licence/copyright headers, and assign third-party notices to the repositories distributing the material. Check relative document links after moves. Repository and crate names can differ; renaming every crate is unnecessary for the first extraction.

**Define compatibility explicitly.** Every extracted repository records supported CPUs/hosts, public versus experimental APIs, verification commands and known limits. Breaking changes need consumer updates and a newly tested distribution combination. Rust source-package versions and loadable-app ABI versions are different contracts.

## Suggested migration slips

These slips define reviewable future work. They do not authorise creating remote repositories, publishing packages, pushing branches or deleting existing paths.

| Slip | Concrete outcome | Completion evidence |
|---|---|---|
| 1. Choose ownership | Select an option and names; identify the shell, hosted executable, native foundation and assembly owners | Agreed ownership map and source-to-destination inventory |
| 2. Harden the seam | Promote the runtime locally; resolve cleanup/mapping contracts and distinguish app/`std` ABIs | Targeted lifecycle checks plus existing DOS consumer checks |
| 3. Prepare an independent kit | Collect runtime, HAL, target/linker/packer assets, examples, docs and provisioning inputs | Clean staged-tree build with no Desk source assumptions |
| 4. Execute Option 1 | Prepare `pc-platform`, pin Desk to its reviewed revision, update build paths and local overrides | Independent checks and both-track frame comparison pass; each moved implementation has one authoritative source |
| 5. Earn Option 2 | Separate `pc-hal`/`dos-rs`; establish native shell composition and move system ownership | Independent low-level checks and complete hosted/native builds from `desk-system` |
| 6. Stabilise app integration | Run a real app in the shell; settle ABI versioning, allocation ownership, unload and supported failures | Lifecycle and compatibility checks against the product host |
| 7. Earn Option 3 selectively | Separate drivers, kernel, SDK and apps as independent maintenance needs emerge | Each moved layer has its own suite and consumer; distribution pins reproduce the tested product |

A slip is complete when its consumer works from the new source of truth. Creating empty repositories or moving folders is not an acceptance result. Keep crates together when their interfaces still require frequent coordinated edits.

## Validation and limits

The ownership maps were checked against current manifests, relative dependencies, runtime location and native/hosted wiring, together with Direction and the integration analysis. Names, publication policies and repository counts are proposals; name availability and external registry state were not checked.

Validation for this documentation change uses the repository Markdown/link checker and whitespace checks. No code, dependencies, build paths or repository settings are changed. No extraction, independent build or cross-repository compatibility claim has been verified. Full hosted/native checks remain a gate for executing the proposed moves.
