| API | stub (unsupported) | trusty (borrowed) | desk (patched) |
|---|---|---|---|
| `println!`, `eprintln!` | compiles, links, runs, **prints nothing** | prints (`writev`) | prints |
| `stdout().write_all`, `flush` | `Ok(())`, discards | writes | writes |
| `stdin().read_line` | `Ok(0)` | `Ok(0)` | reads handle 0 (`Ok(0)` at `NUL`) |
| `Vec`, `String`, `format!`, `Box` | works (the program's own `#[global_allocator]`) | works | works |
| `System.alloc` | **returns null** | non-null (`malloc`) | non-null (`desk_alloc`) |
| `HashMap` | works; the seed comes from stack and heap addresses, so it repeats on every run | works (seed from the shim) | works (seed from the shim) |
| `BTreeMap` | works | works | works |
| `Instant::now`, `elapsed` | **panics** (`time not implemented on this platform`), process aborts | same | works, microsecond steps |
| `SystemTime::now` | panics, aborts | same | works (DOS date and time, 10 ms) |
| `thread::Builder::spawn` | **aborts** (see below) | `Err(Unsupported)` | `Err(Unsupported)` |
| `thread::spawn` | aborts | panics: `failed to spawn thread: Unsupported`, aborts | same, exit 134 |
| `thread::sleep` | panics (`can't sleep`), aborts | same | works (busy-wait; 1000 ms is 18 BIOS ticks) |
| `thread::current()`, `available_parallelism`, `yield_now` | `current()` **aborts**; `available_parallelism` is `Err(NotFound)`; `yield_now` returns | `ThreadId(1)`, `Err(NotFound)`, returns | same as trusty |
| `Mutex`, `RwLock`, `Once`, `OnceLock`, atomics | work (no-contention implementations) | work | work |
| `env::args` | empty | empty | the program path, then the command tail (`[path, "all"]`) |
| `env::var` | `Err(NotPresent)` | `Err(NotPresent)` | works (`COMSPEC`); `vars()` works |
| `env::vars()` | panics (`not supported on this platform`) | panics | works |
| `fs::File::open`, `fs::write` | `Err(Unsupported)` | `Err(Unsupported)` | `Err(Unsupported)` (not implemented) |
| `process::exit(7)` | aborts, status 1 | aborts, status 1 | exits 7 |
| `panic!` | message lost (`panic_output` is `None`), aborts | message on standard error, aborts | message on standard error, status 134 |
| `panic::set_hook` | the hook receives the message (`panicked at ..:\nboom 42`) | same | same |
| `process::abort` | `ud2`, status 1 | same | exit 134 |
| `thread_local!` (`Cell` and `RefCell<Vec>`) | works (plain statics) | works | works |
