# Machine Notation

## 1. Purpose

A **machine** is one or more capability **strands** wired into data-flow graphs and identified together as one anchor-realized value. Machine notation is the textual encoding of such a machine.

It is used for:

1. **Stable identifiers** — A canonical string by which two machine values can be compared, stored, and looked up.
2. **Round-trip serialization** — Persist a machine to text, parse it back, get an equivalent machine.
3. **Human authoring** — A compact, readable form a user can write or edit directly.

This document describes the language. The dispatch and ranking semantics for individual caps are defined in [05-DISPATCH](./07-DISPATCH.md) and [06-RANKING](./08-RANKING.md); machine notation builds on top of them.

---

## 2. Layers

The system distinguishes three layers:

| Layer | Type | Anchor commitment? | Cap-arg assignment? | Inputs from? |
|-------|------|--------------------|---------------------|--------------|
| Planner | `Strand` | no | no | media URN patterns |
| Anchored | `Machine` (this document) | yes | yes (resolved) | anchor URNs |
| Concrete | `MachineRun` | yes | yes | actual input files |

A planner `Strand` is a linear sequence of cap steps with no commitment to which data positions are inputs and which are intermediate. A `Machine` is the **anchor-realized** form: every internal data position is identified, every cap arg is bound to its source position, and the input/output anchors are committed. A `MachineRun` binds those anchors to actual input files at execution time.

---

## 3. Surface Forms

Two equally valid statement forms exist. Both can be freely mixed in the same program.

### 3.1 Bracketed

Each statement is wrapped in `[...]`. Inside a bracketed statement the cap URN reads until the closing `]`, so line breaks between statements are insignificant — multiple statements may share a line:

```
[extract cap:in="media:pdf";extract;out="media:txt;textable"]
[doc -> extract -> text]
```

### 3.2 Line-based

One statement per line, no brackets. In this form a cap URN reads until the newline, so line breaks **terminate** statements:

```
extract cap:in="media:pdf";extract;out="media:txt;textable"
doc -> extract -> text
```

The bracketed form is the canonical serialization (one-line, suitable for use as an identifier). The line-based form is intended for human reading and editing. Both forms can be freely mixed in the same program.

---

## 4. Statement Kinds

A program is a sequence of two kinds of statements: **headers** and **wirings**.

### 4.1 Headers

A header binds an alias to a `CapUrn`:

```
<alias> <cap-urn>
```

Examples:

```
extract cap:in="media:pdf";extract;out="media:txt;textable"
embed cap:in="media:textable";embed;out="media:embedding-vector;record;textable"
```

The cap URN is parsed by `CapUrn::from_string` (see [04-CAP-URN-STRUCTURE](./06-CAP-URN-STRUCTURE.md)). The header is consumed in exactly one place: each matching wiring references it by alias.

### 4.2 Wirings

A wiring connects nodes through a cap:

```
<source> -> <loop_cap> -> <target>
```

`source` is either a single node alias or a parenthesized fan-in group `(a, b, c)`. `loop_cap` is either a header alias or `LOOP <alias>` (sets `is_loop` on the resulting edge). `target` is a single node alias.

Examples:

```
doc -> extract -> text
text -> embed -> vectors
pages -> LOOP extract -> texts
(thumbnail, model_spec) -> describe -> description
```

The grammar is defined in `capdag/src/machine/machine.pest`. Whitespace and line breaks between statements are insignificant in the bracketed form; in the line-based form, line breaks terminate statements.

---

## 5. Aliases and Node Names

**Aliases and node names are opaque labels.** The parser does not extract any meaning from them beyond their identity within the program: two references to the same name refer to the same thing, two different names refer to different things. Conventions like `extract`, `doc`, `text` are for the user's benefit only — the canonical machine produced by the parser does not embed them.

The serializer produces canonical aliases as `edge_<i>` and canonical node names as `n<i>`, where `<i>` is a global counter across all strands. Two semantically-equivalent machines produce byte-identical canonical notation.

The only constraint on alias and node names is the lexical form (`(ALPHA | "_") (ALNUM | "_" | "-")*`) and that no node name may collide with a header alias.

---

## 6. Strands and Connected Components

A `Machine` consists of one or more `MachineStrand`s. A **strand** is a maximal connected sub-graph of the wiring graph: two wirings belong to the same strand iff there exists a path through the wiring set, hopping along shared node names, that connects them. The parser computes connected components on the wirings via union-find and produces one `MachineStrand` per component.

```
[a -> cap_x -> b]
[b -> cap_y -> c]
[d -> cap_z -> e]
```

The first two wirings share node `b` → one strand (a → cap_x → b → cap_y → c). The third wiring shares no node names with the first two → a separate strand (d → cap_z → e).

**Crossings are internal to a single strand.** What looks like "two strands crossing" via a shared node is, by definition, just one strand with a shared internal node. Two strands within the same machine never share node names.

Strand declaration order matters: strands are listed in the resulting `Machine` in **first-appearance order** (the strand whose earliest wiring appears first in the textual input comes first). Strict equivalence (§9) compares strands position-by-position.

---

## 7. Source-to-Cap-Arg Resolution

Each cap defines its input arguments in its `args` list. Each arg has a `media_urn` (the **slot identity**, unique per cap per RULE1) and a `sources` list describing how the arg receives data at runtime. The source types are:

- `Stdin { stdin: "<media URN>" }` — the arg receives data via the stdin stream. The inner URN is the **data-flow type** (e.g. `media:pdf`). This is the type the runtime delivers on the wire.
- `Position { position: N }` — positional CLI argument.
- `CliFlag { cli_flag: "--name" }` — named CLI flag.

**Only args with a Stdin source participate in source-to-cap-arg matching.** Args with only CLI/positional sources are runtime configuration — they receive their values at execution time from cap settings, slot values, or defaults, not from upstream caps in the data flow.

The slot identity (`arg.media_urn`, e.g. `media:file-path;textable`) may differ from the stdin source URN (`media:pdf`). The slot identity is the cap's internal label for the arg slot; the stdin URN is the type of data that actually flows on the wire. The runtime handles the translation transparently (e.g. reading a file path and piping the file's bytes into stdin). The resolver matches against the **stdin source URN**, not the slot identity.

### Matching algorithm

The matching is a **minimum-cost bipartite assignment with a uniqueness check** over the wiring's source URNs and the cap's stdin-source URNs:

- **Cost**. For source URN `s` and stdin arg URN `a`: if `s.conforms_to(a)` is false, the pair is impossible. Otherwise the cost is `spec(s) - spec(a)`, the **specificity distance** (always non-negative because `s ⪯ a` implies `spec(s) ≥ spec(a)`). Smaller distance = the arg is the tightest fit for the source.
- **Algorithm**. Brute-force enumeration of all perfect injections from sources to stdin args. The minimum-cost matching wins.
- **Uniqueness**. The minimum-cost matching must be unique. If two distinct assignments tie at the same minimum total cost, the resolver fails with `AmbiguousMachineNotation`. **Source vec position is not used as a tiebreaker.**
- **Unmatched source**. If any source has no candidate stdin arg (no stdin arg whose URN it conforms to), the resolver fails with `UnmatchedSourceInCapArgs`.

The result is a `Vec<EdgeAssignmentBinding>` of `(cap_arg_media_urn, source_node)` pairs, where `cap_arg_media_urn` is the **slot identity** (the arg's outer `media_urn`, for RULE1-based identification), sorted by `cap_arg_media_urn` for canonical comparison.

Resolution requires the cap registry to look up each cap's `args` list. All `Machine` constructors (`from_strand`, `from_strands`, `from_string`) take `&CapRegistry`.

---

## 8. Anchors

Each `MachineStrand` has two anchor sets:

- **Input anchors**: NodeIds (data positions) that no edge in the strand produces as its target. These are the strand's external inputs — the data the user provides at runtime.
- **Output anchors**: NodeIds that no edge in the strand consumes via any binding. These are the strand's external outputs — the data the strand produces.

Anchors are sorted multisets of `MediaUrn` (sorted by structural `MediaUrn::Ord`); they are compared positionally on the sorted form, which is multiset equality.

A node that is both consumed and produced is an internal data position. A node that is neither cannot exist (every node must appear in at least one wiring, where it is either a source or a target).

---

## 9. Strict Equivalence

`Machine::is_equivalent` is the only equivalence relation on machines. It is **strict, positional**:

1. Same number of `MachineStrand`s in the `strands` vec.
2. For every i, `self.strands[i].is_equivalent(&other.strands[i])`.

`MachineStrand::is_equivalent` walks both strands in their canonical edge order (computed by the resolver via Kahn's topological sort with a structural tiebreaker), comparing edges position-by-position. It builds a `NodeId` bijection between the two strands on the fly: any inconsistency (the same self-NodeId mapped to two different other-NodeIds, or two NodeIds whose URNs are not `is_equivalent`) fails the comparison.

Each edge's `assignment` vec is pre-sorted by cap arg media URN, so positional comparison over assignments is canonical. Two equivalent edges always have identical sorted assignment vecs.

There is no looser variant. If a "drop-in replaceable but order-flexible" relation is ever needed, it will get its own descriptive name and will not be called `is_equivalent`.

---

## 10. Canonical Notation

Two strictly-equivalent `Machine`s produce byte-identical canonical notation, because:

- Strand order is part of the machine's identity (`Machine::is_equivalent` is positional over strands).
- Within each strand, canonical edge order is deterministic (Kahn's algorithm with a structural tiebreaker on `(cap_urn, sorted_assignment, target_node_urn, is_loop)`).
- Aliases are `edge_<i>` from a global counter.
- Node names are `n<i>` from a global counter.
- Cap URNs are written in their canonical `to_string` form.

A user-authored notation, parsed and then re-serialized, produces canonical notation regardless of what aliases / node names the user wrote. Round-tripping the canonical notation is a fixed point.

---

## 11. From Strand to Machine

Three ways to build a `Machine`:

| Constructor | Input | Strand count | Crossings? |
|---|---|---|---|
| `Machine::from_strand(strand, registry)` | one planner `Strand` | 1 | no |
| `Machine::from_strands(strands, registry)` | a slice of planner `Strand`s | N (one per input) | no |
| `Machine::from_string(notation, registry)` | machine notation text | discovered by connected components | yes (within a strand) |

The two programmatic constructors (`from_strand`, `from_strands`) treat each input strand as a self-contained DAG. Even if two input strands have type-compatible URNs internally, `from_strands` does not join them — that's the contract. **Crossings only arise from notation**, where the user explicitly shares node names across wirings.

For each Cap step in a planner strand, `from_strand` builds one resolved edge whose source is the step's `from_spec` and whose target is the step's `to_spec`. `ForEach` sets `is_loop = true` on the next cap edge; `Collect` is elided (cardinality transitions are implicit in the resolved data flow).

### Positional interning (planner path)

The planner chains caps by conformance: a cap's declared input (`in=media:textable`) may be less specific than the preceding cap's declared output (`out=media:page;textable`). At runtime the more-specific data flows through both. The resolver uses **positional interning** to collapse these into one shared data position:

- Each cap step's **source** reuses the preceding cap step's target NodeId iff the two URNs are on the same specialization chain (`is_comparable`). The more-specific URN wins as the canonical representative.
- Each cap step's **target** always allocates a fresh NodeId.
- `ForEach` and `Collect` preserve the boundary position through the cardinality transition.

This ensures that consecutive caps in a strand share an intermediate node even when their declared URNs differ by specificity.

---

## 12. Failure Modes

Building a `Machine` from notation can fail in several distinct ways:

| Error | Source | Meaning |
|---|---|---|
| `MachineSyntaxError::Empty` | parser | Input is empty or whitespace-only. |
| `MachineSyntaxError::ParseError` | parser | Pest grammar parse failed (malformed brackets, unrecognized tokens, etc.). |
| `MachineSyntaxError::UnterminatedStatement` | parser | A bracket `[` was opened but never closed with `]`. |
| `MachineSyntaxError::DuplicateAlias` | parser | Two header statements define the same alias. |
| `MachineSyntaxError::UndefinedAlias` | parser | A wiring references an alias that no header defines. |
| `MachineSyntaxError::NodeAliasCollision` | parser | A wiring uses a node name that is also a header alias. |
| `MachineSyntaxError::InvalidWiring` | parser | Two URNs bound to the same node name are not on the same specialization chain (`is_comparable` returns false). |
| `MachineSyntaxError::InvalidCapUrn` | parser | A cap URN in a header failed to parse. |
| `MachineSyntaxError::InvalidMediaUrn` | parser | A media URN referenced in a cap's `in=` or `out=` spec failed to parse. |
| `MachineSyntaxError::InvalidHeader` | parser | A header statement has invalid structure. |
| `MachineSyntaxError::NoEdges` | parser | The notation has headers but no wirings. |
| `MachineAbstractionError::NoCapabilitySteps` | resolver | The strand or wiring set contains no Cap step. |
| `MachineAbstractionError::UnknownCap` | resolver | A cap referenced by a wiring is not in the cap registry's cache. |
| `MachineAbstractionError::UnmatchedSourceInCapArgs` | resolver | A source URN does not conform to any of the cap's input args. |
| `MachineAbstractionError::AmbiguousMachineNotation` | resolver | The minimum-cost source-to-cap-arg matching is not unique. |
| `MachineAbstractionError::CyclicMachineStrand` | resolver | The resolved data-flow graph of a strand contains a cycle. |

`MachineParseError` is the union returned by `Machine::from_string` and `parse_machine`; it wraps either a `MachineSyntaxError` (lexical / grammatical) or a `MachineAbstractionError` (resolution).

---

## 13. Examples

### 13.1 Linear chain

```
[extract cap:in="media:pdf";extract;out="media:txt;textable"]
[embed cap:in="media:textable";embed;out="media:embedding-vector;record;textable"]
[doc -> extract -> text]
[text -> embed -> vectors]
```

One strand. Three nodes (`doc`, `text`, `vectors`). Two edges. Input anchor: the URN bound to `doc` (= `media:pdf`). Output anchor: the URN bound to `vectors`.

Canonical re-serialization:

```
[edge_0 cap:in="media:pdf";extract;out="media:txt;textable"][edge_1 cap:in="media:textable";embed;out="media:embedding-vector;record;textable"][n0 -> edge_0 -> n1][n1 -> edge_1 -> n2]
```

### 13.2 Strand with iteration

```
[disbind cap:in="media:pdf";disbind;out="media:page;textable"]
[make_decision cap:in="media:textable";make-decision;out="media:decision;json;record;textable"]
[doc -> disbind -> pages]
[pages -> LOOP make_decision -> decisions]
```

One strand. The `LOOP` marker on the second wiring sets `is_loop = true` on the resolved edge — semantically, the cap runs once per item of the sequence. The cap definition declares `make_decision`'s input as `media:textable`, but the source URN is the more-specific `media:page;textable`; the resolver's matching accepts the conforming source and emits the binding `(media:textable, page_node)`.

### 13.3 Fan-out

```
[meta cap:in="media:pdf";extract-metadata;out="media:file-metadata;record;textable"]
[outline cap:in="media:pdf";extract-outline;out="media:document-outline;record;textable"]
[thumb cap:in="media:pdf";generate-thumbnail;out="media:image;png;thumbnail"]
[doc -> meta -> metadata]
[doc -> outline -> outline_data]
[doc -> thumb -> thumbnail]
```

One strand. All three wirings share the source `doc`, so they are connected (one connected component → one strand). Four nodes total: `doc`, `metadata`, `outline_data`, `thumbnail`. Three edges. The input anchor is `doc`'s URN (`media:pdf`); the output anchors are the URNs of `metadata`, `outline_data`, and `thumbnail`.

### 13.4 Fan-in

```
[thumb cap:in="media:pdf";generate-thumbnail;out="media:image;png;thumbnail"]
[model_dl cap:in="media:model-spec;textable";download;out="media:model-spec;textable"]
[describe cap:in="media:image;png";describe-image;out="media:image-description;textable"]
[doc -> thumb -> thumbnail]
[spec_input -> model_dl -> model_spec]
[(thumbnail, model_spec) -> describe -> description]
```

The `describe` cap declares two args in its definition: `media:image;png` and `media:model-spec;textable`. The fan-in wiring `(thumbnail, model_spec) -> describe -> description` provides two source URNs. The resolver's bipartite matching pairs `thumbnail` (URN: `media:image;png;thumbnail`) with the `media:image;png` arg (it conforms) and `model_spec` (URN: `media:model-spec;textable`) with the `media:model-spec;textable` arg. The resulting edge has two `EdgeAssignmentBinding`s.

If the `describe` cap had declared only one arg, the resolver would fail with `UnmatchedSourceInCapArgs` for the second source.

---

## 14. Summary

| Concept | Type | Identity rule |
|---|---|---|
| Strand (planner) | `Strand` | sequence of `StrandStep`s |
| Strand (in machine) | `MachineStrand` | maximal connected component, with anchors |
| Machine | `Machine` | ordered `Vec<MachineStrand>` |
| Edge | `MachineEdge` | cap URN + sorted assignment + target NodeId + is_loop |
| Binding | `EdgeAssignmentBinding` | `(cap_arg_media_urn, source_node_id)` |
| Anchor | `Vec<MediaUrn>` (sorted) | multiset of root or leaf URNs |

The resolved machine is the canonical anchor-realized form. Two strictly-equivalent machines produce byte-identical canonical notation; the canonical notation is a stable identifier for the machine value.
