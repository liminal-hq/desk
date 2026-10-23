#!/bin/bash
# The spike E checks: format and lints, the builds, then every API probe run in headless DOSBox on each std platform layer.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/e-std-stubs/check.sh        (after tools/setup.sh)
#
# The expectations are what the runs showed, tied to the pinned toolchain (docs/E-STD-STUBS.md has the full table). A probe that
# ends the process is run alone; `all` runs the ones that return on every platform.
set -euo pipefail
cd "$(dirname "$0")/../.."
E=spikes/e-std-stubs
export RUSTC_BOOTSTRAP=1
step() { printf '\n== %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }

# run <flavour> <program> [args...]: sets $out (standard output) and $status
run() {
  local dir=dist/e/$1
  shift
  status=0
  out="$(tools/dosrun.sh "$dir" "$@" 2> /dev/null)" || status=$?
}

# expect <label> <status> [text...]: $status must match, and every text must appear in $out
expect() {
  local label=$1 want=$2 text
  shift 2
  [ "$status" = "$want" ] || fail "$label: exit status $status, not $want; output: $out"
  for text in "$@"; do
    case "$out" in *"$text"*) ;; *) fail "$label: output lacks '$text': $out" ;; esac
  done
}

# absent <label> <text>: the text must not appear in $out
absent() {
  case "$out" in *"$2"*) fail "$1: output has '$2': $out" ;; esac
}

step "Rust format and lints (spike workspace)"
(cd $E && cargo fmt --check)

step "Build: the patched std copy, then every flavour (no_std baseline, stub, trusty, desk, dos)"
$E/build.sh 2>&1 | tail -3

step "Lints, on the patched std"
(cd $E && __CARGO_TESTS_ONLY_SRC_ROOT="$PWD/target/desk-src/library" cargo clippy --release --locked --target targets/desk-app.json \
  -p probe -p shim --features probe/desk,shim/desk -- -D warnings)
(cd $E && cargo clippy --release --locked --target targets/dos-trusty.json -p probe -p shim --features probe/trusty,shim/trusty -- -D warnings)
(cd $E && cargo clippy --release --locked --target targets/dos-std.json -p base -Zbuild-std=core,alloc,compiler_builtins -- -D warnings)

step "Unmodified std does not build for an invented OS name (desk, dos, stub)"
for os in desk dos stub; do
  dir="$(mktemp -d "${TMPDIR:-/tmp}/e-std.XXXXXX")"
  sed "s/\"os\": \"dos\"/\"os\": \"$os\"/" $E/targets/dos-std.json > "$dir/t.json"
  status=0
  log="$(cd $E && cargo build --release --target "$dir/t.json" -p probe --target-dir "$dir/target" 2>&1)" || status=$?
  rm -rf "$dir"
  [ "$status" != 0 ] || fail "std built for os \"$os\" without a platform layer"
  for gap in 'System: core::alloc::GlobalAlloc' 'decode_error_kind' 'cannot find function `fill_bytes`' 'thread_local::key::set'; do
    case "$log" in *"$gap"*) ;; *) fail "os \"$os\": the build error lacks '$gap'" ;; esac
  done
  echo "os \"$os\": does not build (missing: System allocator, io error mapping, thread-local keys, fill_bytes)"
done

step "rustc refuses os \"unknown\" outside the targets it is for"
dir="$(mktemp -d "${TMPDIR:-/tmp}/e-std.XXXXXX")"
sed 's/"os": "dos"/"os": "unknown"/' $E/targets/dos-std.json > "$dir/t.json"
status=0
log="$(cd $E && cargo build --release --target "$dir/t.json" -p probe --target-dir "$dir/target" 2>&1)" || status=$?
rm -rf "$dir"
[ "$status" != 0 ] || fail "rustc accepted os unknown"
case "$log" in *'`unknown` os can only be used on particular targets'*) echo "rejected, as it should be" ;; *) fail "unexpected error: $log" ;; esac

step "The no_std baseline"
run base HELLO.EXE
expect "base HELLO" 0 "hello: squares=10 sum=385"

step "Stub platform (os \"stub\", patched std with only what an unknown OS lacks): the unsupported layer"
run stub PROBE.EXE all
expect "stub all" 0 "T println: end" "System.alloc(64) null? true" "stdout().write_all -> Ok(())" "hashmap: len=100 get(42)=Some(84)" \
  "btreemap: len=50" "sync: mutex=5 once ran 1 time(s)" "env::args: 0 []" "env::var(COMSPEC) -> Err(NotPresent)" \
  "fs: File::open(HELLO.EXE) -> Err kind=Unsupported" "thread_local: counter=11" "PROBE all returned"
absent "stub all" "STD-PRINTLN"
absent "stub all" "STD-EPRINTLN"
run stub HELLOSTD.EXE
expect "stub HELLOSTD" 0
absent "stub HELLOSTD" "hello"
for t in thread_info thread_spawn vars instant systemtime sleep spawn_unwrap exit panic abort; do
  run stub PROBE.EXE $t
  expect "stub $t" 1 "T $t: start"
  absent "stub $t" "PROBE $t returned"
done
run stub PROBE.EXE panic_hook
expect "stub panic_hook" 1 "hook saw: panicked at" "boom 42"
run stub PROBE.EXE stdin "<" NUL
expect "stub stdin" 0 'read_line -> Ok(0) ""'
echo "stub: println is silent, System.alloc is null, thread::current and every clock end the process"

step "Borrowed Trusty platform (unmodified std, os \"trusty\", libc-style symbols from the shim)"
run trusty PROBE.EXE all
expect "trusty all" 0 "STD-PRINTLN visible" "STD-EPRINTLN visible" "STD-WRITE_ALL visible" "System.alloc(64) null? false" \
  "hashmap: len=100 get(42)=Some(84)" "sync: mutex=5 once ran 1 time(s) oncelock=init rwlock=3 atomic=3 try_lock while held fails=true" \
  "env::args: 0 []" "env::var(COMSPEC) -> Err(NotPresent)" "fs: File::open(HELLO.EXE) -> Err kind=Unsupported" "PROBE all returned"
run trusty PROBE.EXE thread_info
expect "trusty thread_info" 0 "available_parallelism -> Err(NotFound)" "current().id -> ThreadId(1)"
run trusty PROBE.EXE thread_spawn
expect "trusty thread_spawn" 0 "Builder::spawn -> Err(\"operation not supported on this platform\") kind=Unsupported"
for t in vars instant systemtime sleep spawn_unwrap exit panic panic_hook abort; do
  run trusty PROBE.EXE $t
  expect "trusty $t" 1 "T $t: start"
done
run trusty PROBE.EXE instant
expect "trusty instant" 1 "time not implemented on this platform"
run trusty PROBE.EXE sleep
expect "trusty sleep" 1 "can't sleep"
run trusty PROBE.EXE exit
expect "trusty exit" 1 "calling process::exit(7)"
run trusty HELLOSTD.EXE
expect "trusty HELLOSTD" 0 "hello: squares=10 sum=385"
run trusty HELLOMAP.EXE
expect "trusty HELLOMAP" 0 'hello: [("k0", 0), ("k1", 1)'
echo "trusty: println works through writev; time, sleep, threads, files and process::exit do not"

for flavour in desk dos; do
  step "Patched Desk platform layer (os \"$flavour\"): stdout, alloc, args, env, random, time, sleep and exit over the runtime"
  run $flavour PROBE.EXE all
  expect "$flavour all" 0 "STD-PRINTLN visible" "STD-EPRINTLN visible" "STD-WRITE_ALL visible" "System.alloc(64) null? false" \
    "hashmap: len=100 get(42)=Some(84)" "two RandomState hashes of 1 differ" "env::args: 1 [\"all\"]" "env::var(COMSPEC) -> Ok(" \
    "try_lock while held fails=true" "fs: File::open(HELLO.EXE) -> Err kind=Unsupported" \
    "PROBE all returned"
  run $flavour PROBE.EXE thread_spawn
  expect "$flavour thread_spawn" 0 "Builder::spawn -> Err(\"operation not supported on this platform\") kind=Unsupported"
  run $flavour PROBE.EXE thread_info
  expect "$flavour thread_info" 0 "available_parallelism -> Err(NotFound)" "current().id -> ThreadId(1)"
  run $flavour PROBE.EXE vars
  expect "$flavour vars" 0 "env::vars().count() -> "
  run $flavour PROBE.EXE instant
  expect "$flavour instant" 0 "Instant::now twice: elapsed" "Instant::elapsed -> "
  run $flavour PROBE.EXE systemtime
  expect "$flavour systemtime" 0 "SystemTime::now -> since epoch Ok("
  run $flavour PROBE.EXE sleep
  expect "$flavour sleep" 0
  echo "$out" | grep -Eq 'after (1[7-9]|20) BIOS ticks' || fail "$flavour sleep: not about 18 BIOS ticks for 1000 ms: $out"
  run $flavour PROBE.EXE exit
  expect "$flavour exit" 7 "calling process::exit(7)"
  run $flavour PROBE.EXE panic
  expect "$flavour panic" 134 "index out of bounds: the len is 0 but the index is 3"
  run $flavour PROBE.EXE panic_hook
  expect "$flavour panic_hook" 134 "hook saw: panicked at" "boom 42"
  run $flavour PROBE.EXE spawn_unwrap
  expect "$flavour spawn_unwrap" 134 "failed to spawn thread"
  run $flavour PROBE.EXE abort
  expect "$flavour abort" 134
  run $flavour PROBE.EXE stdin "<" NUL
  expect "$flavour stdin" 0 'read_line -> Ok(0) ""'
  run $flavour HELLOSTD.EXE
  expect "$flavour HELLOSTD" 0 "hello: squares=10 sum=385"
  run $flavour HELLOMAP.EXE
  expect "$flavour HELLOMAP" 0 'hello: [("k0", 0), ("k1", 1)' "took "
  echo "$out" | head -n 1
done

step "Sizes: the same hello program on each platform layer (bytes, stub included)"
printf '%-24s %8s %8s\n' layer HELLO HELLO+map
printf '%-24s %8s %8s\n' "no_std + alloc" "$(wc -c < dist/e/base/HELLO.EXE)" "-"
for f in stub trusty desk; do
  printf '%-24s %8s %8s\n' "std, $f" "$(wc -c < dist/e/$f/HELLOSTD.EXE)" "$(wc -c < dist/e/$f/HELLOMAP.EXE)"
done

printf '\nAll spike E checks passed.\n'
