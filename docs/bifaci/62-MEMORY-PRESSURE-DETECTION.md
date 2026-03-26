# 62 — Memory Pressure Detection

How to detect and respond to memory pressure caused by plugin processes
(inference model loads, large allocations) on macOS, before the system
becomes unresponsive.

## The Problem

When an inference plugin loads a large model (8–14GB GGUF via llama.cpp,
safetensors via candle, MLX weights), macOS enters a series of memory
management stages. If unmanaged, the system freezes and jetsam
(the kernel's OOM killer) fires — but jetsam is unreliable:

1. It may kill the host app instead of the plugin
2. It fires too late — the system is already frozen/unresponsive
3. It provides no notification to our code — the plugin just dies
4. In production, it has been observed to fail entirely

We must detect pressure proactively and kill the offending plugin ourselves.

## macOS Memory Management Stages

When a process allocates memory aggressively, macOS responds in stages:

### Stage 1: Active Pages (seconds 0–3 typically)

New allocations create **active anonymous pages**. The `active_count` in
`vm_statistics64` rises. `available` (total − active − wired) drops.
The system is fine — this is normal allocation.

**Metrics**: active rises, available drops, compressed stable, swap stable.

### Stage 2: Compression (seconds 3–5 typically)

When active pages exceed physical RAM capacity, the kernel's **WKdm
compressor** activates. It compresses inactive pages in-place, freeing
physical page frames. `compressor_page_count` rises sharply —
potentially 6–10GB in 1–2 seconds on Apple Silicon.

**Key insight**: "available" (total − active − wired) **rises** during
compression because compressed pages are counted differently. Available
is useless as a pressure signal — it stays at 11–13GB on a 16GB machine
while the system is actively dying.

**Metrics**: compressed explodes, available rises (misleading), active drops.

### Stage 3: Swap Overflow (seconds 4–6 typically)

When the compressor can't keep up with allocation rate, it **spills to
swap** (disk-backed paging). `vm.swapusage` (via sysctl) starts growing.
This is the critical transition — once swap is growing, the system is
degrading rapidly. Disk I/O for swap competes with app I/O.

**Metrics**: swap growing, compressed high, system becoming sluggish.

### Stage 4: System Freeze (seconds 6–10 typically)

The kernel is simultaneously:
- Faulting new pages for the allocating process
- Compressing old pages
- Writing compressed pages to swap
- Reading swapped pages back when accessed

All threads (including our watchdog) get deprioritized. The monitoring
thread that was polling every 250ms can't execute for 30–60+ seconds.
User interaction freezes.

### Stage 5: Jetsam Kill (seconds 10–60+ after freeze)

The kernel's jetsam subsystem eventually selects a process to kill. This
is unreliable — it may kill the wrong process, fire too late, or not
fire at all.

## What Doesn't Work

### "Available Memory" Threshold

```
// DOES NOT WORK
if info.availableMb < 1024 { kill() }
```

Available = total − active − wired. When compression activates, active
pages become compressed and are no longer counted as active. Available
rises back to 11–13GB even while the system is dying. In testing,
available never dropped below 4.7GB even when jetsam killed the process.

### Absolute Compressed Threshold (percentage of RAM)

```
// DOES NOT WORK — false positives
if compressedDeltaPct > 40 { kill() }
```

The kernel compresses proactively as normal memory management. On a 16GB
machine, 30% compressed delta (4.8GB) is routine and the system is
perfectly healthy (Activity Monitor green). Fixed percentage thresholds
are not general-purpose — they must be tuned per machine size, existing
workload, and compression ratio.

### mlock at Scale

```
// DOES NOT WORK — fails at large sizes
libc::mlock(ptr, 12_000_000_000); // errno=35 (EAGAIN)
```

`mlock()` wires pages into physical RAM (prevents compression/swap), but
the process `RLIMIT_MEMLOCK` ulimit is too low for multi-GB allocations.
Raising the ulimit requires root. mlock works for small allocations but
is not a general solution.

### xorshift64 as "Random" Data

```
// DOES NOT WORK — WKdm compresses it
for i in 0..u64_count {
    *(ptr.add(i * 8) as *mut u64) = xorshift64(&mut state);
}
```

macOS uses the WKdm page compressor, which operates on 32-bit word
patterns — not entropy. xorshift64 output has repeating 32-bit patterns
that WKdm exploits. In testing, the kernel compressed 8GB of xorshift64
data in 3 seconds, reducing memory pressure to nothing.

### Allocate-Then-Hold Pattern

```
// DOES NOT WORK — kernel catches up during hold
let buf = vec![0xAA; size_bytes];
thread::sleep(Duration::from_secs(30)); // kernel compresses everything
```

Any pause in allocation gives the kernel time to compress and swap. In
testing, stopping allocation for even 1 second allowed the kernel to
compress 6GB and recover available memory. The compressor runs at
~2–3GB/s on Apple Silicon.

### Sweep Threads Rewriting Pages

```
// DOES NOT WORK — compressor is faster than sweepers
// 4 threads rewriting 3GB each = ~1.5s per full sweep
// Kernel compresses in ~0.5s per 3GB
```

Even with 4 threads continuously rewriting pages, the compressor outruns
them. By the time a thread returns to re-dirty a page, it's already been
compressed. The kernel processes pages faster than user-space can write them.

## What Works: Continuous Allocation Attack

The only approach that creates sustained, realistic pressure is
**continuous allocation without pausing**:

```rust
while start.elapsed() < deadline {
    let addr = libc::mmap(
        null_mut(), chunk_bytes,
        PROT_READ | PROT_WRITE,
        MAP_ANONYMOUS | MAP_PRIVATE,
        -1, 0,
    );
    arc4random_buf(addr, chunk_bytes);  // fault + fill
    regions.push((addr, chunk_bytes));
}
```

This works because:

1. Each `mmap` + write creates **new page faults** the kernel must handle
   synchronously
2. `arc4random_buf` (ChaCha20 CSPRNG) produces output that is genuinely
   incompressible — but the kernel still compresses it (WKdm achieves
   ~1.5:1 even on random data by exploiting 32-bit word alignment)
3. The key is **never stopping** — the kernel can compress any static
   allocation given time, but it can't compress pages that haven't been
   faulted in yet
4. Allocation rate (~2GB/s) competes with compression rate (~2–3GB/s),
   creating a sustained fight that eventually overwhelms the system

This is what actually happens when llama.cpp loads a model: continuous
allocation (vm_allocate for Metal buffers) + memcpy from mmap'd model
file, at multi-GB/s rates.

## Detection Algorithm: Sustained Swap Growth

The correct detection signal is **sustained swap growth** — swap
increasing across 3 or more consecutive polls.

### Why Swap Growth

Swap growth is the universal signal that the compressor has overflowed:

- It's independent of total RAM (works on 8GB, 16GB, 64GB, 128GB)
- It's independent of compression ratio
- It's independent of workload type
- It means the kernel has exhausted in-memory compression capacity and
  is writing to disk
- It precedes system freeze by 2–5 seconds (the critical window)
- It's not triggered by normal compression (compression without swap
  growth is routine and healthy)

### Why Sustained (3+ Polls)

A single swap write can happen during normal operation (page daemon
cleaning up). Two consecutive growths could be a burst. Three consecutive
polls (750ms at 250ms intervals) means the kernel is continuously
writing to swap — the compressor is losing the fight.

### Algorithm

```swift
let pollInterval: TimeInterval = 0.25  // 250ms
var prevSwap = baseline.swapUsedMb
var consecutiveSwapGrowth = 0

// Poll loop
let info = SystemMemoryInfo.current()
if info.swapUsedMb > prevSwap {
    consecutiveSwapGrowth += 1
} else {
    consecutiveSwapGrowth = 0
}
prevSwap = info.swapUsedMb

if consecutiveSwapGrowth >= 3 {
    // Compressor overflowed — kill the plugin NOW
    killPlugin()
}
```

### Supplementary Signals

1. **Kernel pressure dispatch source** (`DISPATCH_SOURCE_TYPE_MEMORYPRESSURE`)
   — fires on `.warning` / `.critical`. Acts as a second layer. In
   testing, this rarely fires before jetsam (the kernel sometimes skips
   the advisory and kills directly), but when it does fire, it's a
   valid signal.

2. **Swap delta from baseline** — all swap measurements are relative to
   baseline at test/session start, to ignore pre-existing swap from
   prior operations.

### What NOT To Use

- `available` (total − active − wired) — rises during compression, useless
- Absolute compressed thresholds — not general-purpose
- Fixed percentage-of-RAM thresholds — don't scale across machine sizes
- `getAvailableMemoryMb()` (free + purgeable) — reports ~500MB during
  normal operation on 16GB machine, triggers false positives

## Implementation: SystemMemoryInfo

```swift
struct SystemMemoryInfo {
    let totalMb: UInt64
    let availableMb: UInt64     // total - active - wired (unreliable!)
    let activeMb: UInt64
    let wiredMb: UInt64
    let compressedMb: UInt64
    let swapUsedMb: UInt64      // via sysctl("vm.swapusage")

    static func current() -> SystemMemoryInfo {
        // vm_statistics64 via host_statistics64(HOST_VM_INFO64)
        // Total via sysctl("hw.memsize")
        // Swap via sysctl("vm.swapusage") → xsw_usage.xsu_used
    }
}
```

The `vm_statistics64` fields:
- `active_count` — pages recently accessed (in use)
- `inactive_count` — pages not recently accessed (candidates for compression)
- `wire_count` — pages locked in RAM (kernel, mlock'd)
- `compressor_page_count` — pages held in the compressor
- `free_count` — truly free pages (usually very low)

## Implementation: Production OOM Watchdog

The production watchdog in `HeartbeatService.swift` has two layers:

### Layer 1: Timer-Based Polling

A 1-second timer (`oomWatchdogTick`) that polls `SystemMemoryInfo` and
checks for sustained swap growth. When triggered, kills inference plugins
via the appropriate path (XPC plugins via `pluginMonitor.killPlugin()`,
engine plugins via `KillPlugins` gRPC RPC).

### Layer 2: Kernel Pressure Dispatch Source

```swift
DispatchSource.makeMemoryPressureSource(
    eventMask: [.warning, .critical],
    queue: .global(qos: .userInteractive)
)
```

Fires before jetsam (when it fires at all). Triggers immediate plugin
kill via `killInferencePluginsNow()`.

### Kill Routing

Plugins are hosted in different contexts:
- **XPC plugins**: killed via `pluginMonitor.killPlugin()` (XPC service
  is the parent process — kill succeeds)
- **Engine plugins**: killed via `KillPlugins` gRPC RPC (engine is the
  parent — `PluginProcessHandle.kill_plugin()` succeeds)
- **Direct kill()**: fails with EPERM for sandboxed/engine-hosted
  plugins (sandbox blocks cross-process signals)

## Timing Characteristics (Apple Silicon, 16GB)

From empirical testing:

| Event | Time | Signal |
|-------|------|--------|
| Allocation starts | 0s | — |
| Active pages fill RAM | 3s | available drops (but recovers) |
| Compressor activates | 3–4s | compressed delta spikes |
| Swap overflow begins | 4–5s | **swap starts growing** |
| Sustained swap growth | 5–6s | **3+ consecutive polls → KILL** |
| System freeze | 7–10s | monitoring thread can't run |
| Jetsam kill | 10–60+s | unreliable |

The window between "detectable" (sustained swap growth) and "system
freeze" is approximately **2–4 seconds**. The 250ms polling interval
with 3-poll confirmation gives a ~750ms detection latency, leaving
1–3 seconds of margin to execute the kill before the system freezes.

## Testcartridge Host

The test harness (`capdag-objc/testcartridge-host/`) validates this
algorithm by:

1. Spawning `testcartridge` as a plugin via `PluginHost`
2. Invoking the `test-memory-hog` cap (continuous allocation attack)
3. Monitoring with the same algorithm (250ms polls, swap growth tracking)
4. Killing the plugin proactively
5. Verifying the plugin is dead and the system recovered

**PASS condition**: We detected pressure and killed the plugin before
jetsam did.

**FAIL condition**: Jetsam killed the plugin before we detected — means
our thresholds need tightening.

## Summary of Pitfalls

1. **"Available" is useless** — rises during compression
2. **Percentage thresholds don't generalize** — 40% of RAM means different
   things on different machines
3. **Compressed delta alone is not dangerous** — compression is normal
4. **Any pause in allocation = kernel wins** — never stop allocating in tests
5. **xorshift/PRNG is compressible by WKdm** — use arc4random_buf (CSPRNG)
6. **Sweep threads can't outrun the compressor** — kernel is faster
7. **mlock fails at scale** — ulimit too low without root
8. **The monitoring thread freezes too** — when the system is under extreme
   pressure, your watchdog can't run (observed: 58-second gap in 250ms polls)
9. **Jetsam skips advisory notifications** — kernel pressure dispatch source
   may not fire before jetsam kills
10. **Swap from prior runs persists** — always use delta from baseline
