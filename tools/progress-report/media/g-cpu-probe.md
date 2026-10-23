| Instruction | QEMU `-cpu 486` | Bochs 486 |
|---|---|---|
| `rdtsc`, `rdmsr`, `emms`, `mov eax, cr4` | executes | `#UD` |
| `cmov`, `cmpxchg8b`, `ud2` | `#UD` | `#UD` |
| `cpuid`, `cmpxchg`, `bswap`, `xadd`, `invlpg`, `wbinvd` | executes | executes |
