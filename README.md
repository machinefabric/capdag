# CapDag - Cap Namespace System

A capability URN and definition system for cartridge architectures, built on [Tagged URNs](https://github.com/machinefabric/tagged-urn-rs).

## Overview

CapDAG provides a formal system for defining, matching, and managing capabilities across distributed cartridge systems. It extends Tagged URNs with:

- **Required direction specifiers** (`in`/`out`) for input/output media types
- **Media URN validation** for type-safe capability contracts
- **Capability registries** for provider discovery and selection
- **Schema validation** for capability arguments and outputs

The system is designed for scenarios where:
- Multiple providers can implement the same capability
- Capability selection should prioritize specificity
- Runtime capability discovery and validation is required
- Cross-language compatibility is needed

## Cap URN Format

Cap URNs extend Tagged URNs with required direction specifiers:

```
cap:in="media:void";generate;out="media:object"
cap:in="media:binary";extract;out="media:object";target=metadata
```

**Direction Specifiers:**
- `in` - Input media type (what the capability accepts)
- `out` - Output media type (what the capability produces)
- Values are Media URNs or wildcard `*`

**Common Tags:**
- `op` - The operation (e.g., `extract`, `generate`, `convert`)
- `target` - What the operation targets (e.g., `metadata`, `thumbnail`)
- `ext` - File extension for format-specific capabilities

For base Tagged URN format rules (case handling, quoting, wildcards, etc.), see [Tagged URN RULES.md](https://github.com/machinefabric/tagged-urn-rs/blob/main/docs/RULES.md).

## Cap Definitions

Full capability definitions include metadata, arguments, and output schemas:

```rust
pub struct Cap {
    pub id: CapUrn,
    pub version: String,
    pub description: Option<String>,
    pub metadata: HashMap<String, String>,
    pub command: String,
    pub arguments: CapArguments,
    pub output: Option<CapOutput>,
    pub stdin: Option<String>,
}
```

**Key Fields:**
- `id` - The cap URN with direction specifiers
- `command` - CLI command or method name for execution
- `arguments` - Required and optional argument definitions with validation
- `output` - Output schema and type information
- `stdin` - If present, the media URN that stdin expects (e.g., "media:bytes;ext=pdf"). Absence means cap doesn't accept stdin.

## Language Implementations

### Rust (`capdag`)

```rust
use capdag::{CapUrn, Cap, CapUrnBuilder};

// Create cap URN
let cap = CapUrn::from_string(
    "cap:in=\"media:binary\";extract;out=\"media:object\";target=metadata"
)?;

// Build with builder pattern
let cap = CapUrnBuilder::new()
    .in_spec("media:binary")
    .out_spec("media:object")
    .tag("op", "extract")
    .tag("target", "metadata")
    .build()?;
```

### Go (`capdag-go`)

```go
import "github.com/machfab/capdag-go"

// Create cap URN
cap, err := capdag.NewCapUrnFromString(
    `cap:in="media:binary";extract;out="media:object"`)

// Build with builder pattern
cap, err = capdag.NewCapUrnBuilder().
    InSpec("media:binary").
    OutSpec("media:object").
    Tag("op", "extract").
    Build()
```

### Objective-C (`capdag-objc`)

```objc
#import "CSCapUrn.h"

// Create cap URN
NSError *error;
CSCapUrn *cap = [CSCapUrn fromString:
    @"cap:in=\"media:binary\";extract;out=\"media:object\""
    error:&error];

// Build with builder pattern
CSCapUrnBuilder *builder = [CSCapUrnBuilder builder];
[builder inSpec:@"media:binary"];
[builder outSpec:@"media:object"];
[builder tag:@"op" value:@"extract"];
CSCapUrn *cap = [builder build:&error];
```

## Capability Matching

Capabilities match requests based on per-tag value semantics:

| Pattern Value | Meaning | Instance Missing | Instance=v | Instance=x≠v |
|---------------|---------|------------------|------------|--------------|
| (missing) | No constraint | OK | OK | OK |
| `K=?` | No constraint (explicit) | OK | OK | OK |
| `K=!` | Must-not-have | OK | NO | NO |
| `K=*` | Must-have, any value | NO | OK | OK |
| `K=v` | Must-have, exact value | NO | OK | NO |

```rust
let provider = CapUrn::from_string(
    "cap:in=\"media:binary\";extract;out=\"media:object\";ext=pdf")?;
let request = CapUrn::from_string(
    "cap:in=\"media:binary\";extract;out=\"media:object\"")?;

// For dispatch/routing, use is_dispatchable
if provider.is_dispatchable(&request) {
    println!("Provider can dispatch this request");
}
```

Specificity uses graded scoring (exact=3, must-have-any=2, must-not-have=1, unspecified=0):

```rust
let general = CapUrn::from_string("cap:in=*;extract;out=*")?;        // specificity: 3+2+2 = 7
let specific = CapUrn::from_string(
    "cap:in=\"media:binary\";extract;out=\"media:object\"")?;        // specificity: 3+3+3 = 9

// specific.specificity() > general.specificity()
```

## Standard Capabilities

Common capability patterns:

**Document Processing:**
- `cap:in="media:binary";extract;out="media:object";target=metadata`
- `cap:in="media:binary";generate;out="media:binary";target=thumbnail`

**AI/ML Inference:**
- `cap:in="media:text";generate;out="media:object";target=embeddings`
- `cap:in="media:object";conversation;out="media:object"`

## Integration

### Provider Registration

```rust
let cap = CapUrn::from_string("cap:in=...;extract;out=...;ext=pdf")?;
provider_registry.register("pdf-provider", cap);

// Find best provider
let caller = provider_registry.can("cap:in=...;extract;out=...")?;
let result = caller.call(args).await?;
```

### CapBlock (Multi-Provider)

```rust
let cube = CapBlock::new();
cube.register_cap_set("provider-a", caps_a);
cube.register_cap_set("provider-b", caps_b);

// Automatically selects best provider by specificity
let (provider, cap) = cube.find_best_match(&request)?;
```

## Documentation

- [RULES.md](docs/RULES.md) - Cap URN specification (cap-specific rules)
- [MATCHING.md](docs/MATCHING.md) - Matching semantics
- [ARCHITECTURE.md](docs/ARCHITECTURE.md) - System architecture
- [MEDIA_DEF_SYSTEM.md](docs/MEDIA_DEF_SYSTEM.md) - Media definition system
- [PERFORMANCE.md](docs/PERFORMANCE.md) - Cross-language throughput measurements
- [Tagged URN RULES.md](https://github.com/machinefabric/tagged-urn-rs/blob/main/docs/RULES.md) - Base URN format rules

## Cross-Language Compatibility

This Rust implementation is the reference. Identical implementations exist for:
- [Go implementation](https://github.com/machinefabric/capdag-go)
- [JavaScript implementation](https://github.com/machinefabric/capdag-js)
- [Objective-C implementation](https://github.com/machinefabric/capdag-objc)

All implementations pass the same test cases and follow identical rules.

## Testing

```bash
cargo test
```

## Performance

Tests conducted on a MacBook M1 Pro (2021) with 16GB RAM running macOS Tahoe 26.3.1 (a), using the identity cap. Each host language (Rust, Go, Python, Swift) was tested with cartridges implemented in each of the four languages, measuring throughput in streaming MB/s.

### Throughput Matrix (MB/s) — Router: Rust

| host \ cartridge | rust | go | python | swift |
|---|---:|---:|---:|---:|
| **rust** | 30.66 | 87.83 | 1.91 | 69.12 |
| **go** | 48.37 | 92.97 | 1.93 | 73.15 |
| **python** | -- | -- | -- | -- |
| **swift** | 45.30 | 103.75 | 2.05 | 76.63 |

### Throughput Matrix (MB/s) — Router: Swift

| host \ cartridge | rust | go | python | swift |
|---|---:|---:|---:|---:|
| **rust** | 64.76 | 102.05 | 1.86 | 71.33 |
| **go** | 58.31 | 92.35 | 1.89 | 62.83 |
| **python** | -- | -- | -- | -- |
| **swift** | 78.64 | 89.90 | 1.74 | 70.26 |

### Ranking (fastest to slowest)

| # | router-host-cartridge | MB/s |
|---:|---|---:|
| 1 | rust-swift-go | 103.75 |
| 2 | swift-rust-go | 102.05 |
| 3 | rust-go-go | 92.97 |
| 4 | swift-go-go | 92.35 |
| 5 | swift-swift-go | 89.90 |
| 6 | rust-rust-go | 87.83 |
| 7 | swift-swift-rust | 78.64 |
| 8 | rust-swift-swift | 76.63 |
| 9 | rust-go-swift | 73.15 |
| 10 | swift-rust-swift | 71.33 |
| 11 | swift-swift-swift | 70.26 |
| 12 | rust-rust-swift | 69.12 |
| 13 | swift-rust-rust | 64.76 |
| 14 | swift-go-swift | 62.83 |
| 15 | swift-go-rust | 58.31 |
| 16 | rust-go-rust | 48.37 |
| 17 | rust-swift-rust | 45.30 |
| 18 | rust-rust-rust | 30.66 |
| 19 | rust-swift-python | 2.05 |
| 20 | rust-go-python | 1.93 |
| 21 | rust-rust-python | 1.91 |
| 22 | swift-go-python | 1.89 |
| 23 | swift-rust-python | 1.86 |
| 24 | swift-swift-python | 1.74 |

```
  swift-swift-python            █                                                1.74 MB/s
  swift-rust-python             █                                                1.86 〃
  swift-go-python               █                                                1.89 〃
  rust-rust-python              █                                                1.91 〃
  rust-go-python                █                                                1.93 〃
  rust-swift-python             █                                                2.05 〃
  rust-rust-rust                █████████████                                   30.66 〃
  rust-swift-rust               ████████████████████                            45.30 〃
  rust-go-rust                  █████████████████████                           48.37 〃
  swift-go-rust                 █████████████████████████                       58.31 〃
  swift-go-swift                ███████████████████████████                     62.83 〃
  swift-rust-rust               ████████████████████████████                    64.76 〃
  rust-rust-swift               ██████████████████████████████                  69.12 〃
  swift-swift-swift             ██████████████████████████████                  70.26 〃
  swift-rust-swift              ███████████████████████████████                 71.33 〃
  rust-go-swift                 ████████████████████████████████                73.15 〃
  rust-swift-swift              █████████████████████████████████               76.63 〃
  swift-swift-rust              ██████████████████████████████████              78.64 〃
  rust-rust-go                  ██████████████████████████████████████          87.83 〃
  swift-swift-go                ███████████████████████████████████████         89.90 〃
  swift-go-go                   ████████████████████████████████████████        92.35 〃
  rust-go-go                    ████████████████████████████████████████        92.97 〃
  swift-rust-go                 ████████████████████████████████████████████   102.05 〃
  rust-swift-go                 █████████████████████████████████████████████  103.75 〃
```

## License

MIT License
