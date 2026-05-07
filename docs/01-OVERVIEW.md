# capdag Specification

## Scope

This specification defines the semantic foundations, runtime protocol, execution model, and development patterns for the capdag system.

---

## Document Map

### Foundations (2-5)

| # | Document | Purpose | Dependencies |
|---|----------|---------|--------------|
| 2 | [Formal Foundations](./02-FORMAL-FOUNDATIONS.md) | Mathematical foundation, dispatch relation | None |
| 3 | [Tagged URN Domain](./03-TAGGED-URN-DOMAIN.md) | Base domain U, normalization, wildcard truth table | 2 |
| 4 | [Predicates](./04-PREDICATES.md) | Derived predicates (accepts, conforms_to, is_comparable, is_equivalent) | 3 |
| 5 | [Specificity](./05-SPECIFICITY.md) | Specificity scoring function | 3 |

### Dispatch and Routing (6-8)

| # | Document | Purpose | Dependencies |
|---|----------|---------|--------------|
| 6 | [Cap URN Structure](./06-CAP-URN-STRUCTURE.md) | Cap URN as product C = U x U x U | 3, 5 |
| 7 | [Dispatch](./07-DISPATCH.md) | The dispatch predicate | 3, 4, 6 |
| 8 | [Ranking](./08-RANKING.md) | Selection among valid providers | 5, 7 |

### Machine Notation and Data Types (9-11)

| # | Document | Purpose | Dependencies |
|---|----------|---------|--------------|
| 9 | [Machine Notation](./09-MACHINE-NOTATION.md) | Textual encoding of multi-cap data-flow graphs | 4, 6 |
| 10 | [Validation Rules](./10-VALIDATION-RULES.md) | Structural validation constraints | 3, 6 |
| 11 | [Media URNs](./11-MEDIA-URNS.md) | Media URN structure and coercion | 3 |

### Protocol (12)

| # | Document | Purpose | Dependencies |
|---|----------|---------|--------------|
| 12.1 | [Architecture](./12.1-ARCHITECTURE.md) | System topology, component roles | 7, 8 |
| 12.2 | [Frame Protocol](./12.2-FRAME-PROTOCOL.md) | Wire format, frame types, size limits | 12.3, 12.4 |
| 12.3 | [Handshake](./12.3-HANDSHAKE.md) | Connection setup, HELLO exchange, identity verification | — |
| 12.4 | [Streaming](./12.4-STREAMING.md) | Multiplexed streams, chunking, sequence numbering | — |

### Cartridge Runtime (13)

| # | Document | Purpose | Dependencies |
|---|----------|---------|--------------|
| 13.1 | [Cartridge Runtime](./13.1-CARTRIDGE-RUNTIME.md) | Entry point, handler registration, CLI/cartridge mode | 7, 12.3 |
| 13.2 | [Input and Output](./13.2-INPUT-OUTPUT.md) | InputStream, OutputStream, stream lookup | 13.4, 15.4 |
| 13.3 | [Peer Invocation](./13.3-PEER-INVOCATION.md) | Cross-cartridge capability calls | — |
| 13.4 | [Progress and Logging](./13.4-PROGRESS-AND-LOGGING.md) | LOG frames, progress mapping, keepalive | 15.3 |

### Host and Relay (14)

| # | Document | Purpose | Dependencies |
|---|----------|---------|--------------|
| 14.1 | [Host Runtime](./14.1-HOST-RUNTIME.md) | Cartridge lifecycle, frame routing, health monitoring | 12.3 |
| 14.2 | [Relay Switch](./14.2-RELAY-SWITCH.md) | Cap-aware routing multiplexer | 7, 8, 15.4 |
| 14.3 | [Relay Topology](./14.3-RELAY-TOPOLOGY.md) | RelaySlave/RelayMaster pairs, Unix socket relay chains | — |

### Execution (15)

| # | Document | Purpose | Dependencies |
|---|----------|---------|--------------|
| 15.1 | [Orchestrator](./15.1-ORCHESTRATOR.md) | Machine notation parsing, DAG construction | 11 |
| 15.2 | [Execution](./15.2-EXECUTION.md) | execute_dag, execute_fanin, topological sort | 15.3 |
| 15.3 | [Progress Mapping](./15.3-PROGRESS-MAPPING.md) | Deterministic progress subdivision | — |
| 15.4 | [Planner](./15.4-PLANNER.md) | Path finding, LiveCapFab, MachinePlan | 15.1 |

### Cartridge Development (16)

| # | Document | Purpose | Dependencies |
|---|----------|---------|--------------|
| 16.1 | [Cartridge Anatomy](./16.1-CARTRIDGE-ANATOMY.md) | Cartridge structure, manifest, cap definitions | 12.3 |
| 16.2 | [Handler Patterns](./16.2-HANDLER-PATTERNS.md) | Op trait, argument extraction, output emission | 15.4 |
| 16.3 | [Model Cartridges](./16.3-MODEL-CARTRIDGES.md) | ML cartridges, three-phase architecture | 13.4 |
| 16.4 | [Content Cartridges](./16.4-CONTENT-CARTRIDGES.md) | Document processing, standard cap patterns | 7, 16.2 |
| 16.5 | [Rust vs Swift](./16.5-RUST-VS-SWIFT.md) | Implementation differences, module coverage | — |

### Integration (17)

| # | Document | Purpose | Dependencies |
|---|----------|---------|--------------|
| 17.1 | [Task Integration](./17.1-TASK-INTEGRATION.md) | Cartridge execution in MachineFabric tasks | 15.4 |
| 17.2 | [Error Handling](./17.2-ERROR-HANDLING.md) | Error type hierarchy, propagation patterns | 13.4 |
| 17.3 | [Memory Pressure Detection](./17.3-MEMORY-PRESSURE-DETECTION.md) | macOS memory pressure, cartridge lifecycle | — |

---

## Reading Order

1. **02 Formal Foundations** — mathematical foundation (optional, for formal reference)
2. **03 Tagged URN Domain** — the base domain
3. **04 Predicates** — the four derived predicates
4. **05 Specificity** — scoring
5. **06 Cap URN Structure** — how Cap URNs compose three dimensions
6. **07 Dispatch** — the central routing rule
7. **08 Ranking** — selection among valid providers
8. **09 Machine Notation** — wire multiple caps into a data-flow graph
9. **10 Validation Rules** — structural constraints
10. **11 Media URNs** — media type details
11. **12 Protocol** — wire format and connection setup
12. **13 Cartridge Runtime** — writing handlers
13. **14 Host and Relay** — cartridge hosting infrastructure
14. **15 Execution** — DAG orchestration and planning
15. **16 Cartridge Development** — building cartridges
16. **17 Integration** — tasks, errors, resource management

---

## Terminology

| Term | Definition |
|------|------------|
| **Tagged URN** | A URN with structure `prefix:key1=value1;key2=value2;...` |
| **Media URN** | A Tagged URN with prefix `media:` describing a data type |
| **Cap URN** | A Tagged URN with prefix `cap:` describing a capability |
| **Pattern** | A URN used as a template or constraint |
| **Instance** | A URN representing a concrete value or request |
| **Provider** | A registered capability that can handle requests |
| **Request** | A capability URN describing what is needed |
| **Dispatch** | The act of routing a request to a valid provider |
| **Specificity** | A numeric score measuring how constrained a URN is |
| **Wildcard** | A special value (`*`, `?`, `!`) with matching semantics |
| **Machine** | An ordered collection of MachineStrands wired into data-flow graphs |
| **MachineStrand** | A maximal connected component of resolved cap edges with anchors |
| **Machine notation** | The textual encoding of a Machine (see [09-MACHINE-NOTATION](./09-MACHINE-NOTATION.md)) |
| **Cartridge** | A standalone binary that provides one or more capabilities |
| **Cap Kind** | A logical classification of a cap into one of *Identity*, *Source*, *Sink*, *Effect*, or *Transform*, derived from `(in, out, y)`. See [06-CAP-URN-STRUCTURE §4](./06-CAP-URN-STRUCTURE.md#4-cap-kinds). |
| **Top type** | `media:` — the universal wildcard for media URNs. Every media URN conforms to it. A side typed `media:` reads as "any A." |
| **Unit type** | `media:void` — the nullary value. A side typed `media:void` reads as "()": no meaningful data flows there. Distinct from the top type. **Atomic**: refinements like `media:void;text` are parse errors. |

---

## Notational Conventions

### Order-Theoretic Notation

- `a <=  b` — "a is at least as specific as b" (a refines b)
- `a >= b` — "a is at least as general as b" (equivalent to b <= a)

### Cap URN Components

A Cap URN `c` is written as a triple:
```
c = (i, o, y)
```
where:
- `i` = input media URN (the `in` tag value)
- `o` = output media URN (the `out` tag value)
- `y` = non-direction cap-tags (op, ext, model, etc. — all arbitrary, none has functional privilege)

---

## Foundation

This specification builds on the mathematical foundations in [02-FORMAL-FOUNDATIONS](./02-FORMAL-FOUNDATIONS.md), which defines the core dispatch relation:

```
Dispatch(p, r) <=> i_r <= i_p /\ o_p <= o_r /\ y_r <= y_p
```

The numbered documents fill in operational details: wildcard truth tables, normalization rules, specificity scoring, validation constraints, machine notation, protocol, runtime, execution, and development patterns.

---

## Conformance

An implementation conforms to this specification if:

1. All Tagged URNs normalize identically (per §3)
2. All predicates compute identically (per §4)
3. Specificity scores match (per §5)
4. Dispatch validity matches (per §7)
5. All validation rules are enforced (per §10)

Ranking policy (§8) may vary by subsystem, but dispatch validity must not.
