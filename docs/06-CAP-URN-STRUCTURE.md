# Cap URN Structure

## 1. Product Structure

A Cap URN is a **triple** over the Tagged URN domain:

```
C = U × U × U
```

For a Cap URN `c ∈ C`:

```
c = (i, o, y)
```

Where:
- `i ∈ U` — Input dimension (the `in` tag value, a Media URN)
- `o ∈ U` — Output dimension (the `out` tag value, a Media URN)
- `y ∈ U` — Non-direction cap-tags (op, ext, model, language, etc.)

---

## 2. String Representation

### 2.1 Canonical Form

A Cap URN serializes as:
```
cap:in="<media-urn>";out="<media-urn>";<cap-tags>
```

Examples:
```
cap:in=media:;out=media:
cap:in="media:pdf";extract;out="media:object"
cap:in="media:textable;form=scalar";prompt;out="media:textable;form=map"
```

### 2.2 Direction Tags

The `in` and `out` tags are **required** in the canonical form:

| Tag | Purpose | Default |
|-----|---------|---------|
| `in` | Input media type | `media:` (any) |
| `out` | Output media type | `media:` (any) |

### 2.3 Non-Direction Tags

All other tags form the `y` dimension. **No tag in `y` has functional
meaning to the protocol** — only `in` and `out` participate in
dispatch, conformance, and ranking. Cap-tags are arbitrary descriptive
metadata: they refine the cap's identity (so two caps with different
`y` are different caps), but no tag key is privileged. Common
descriptive tags include operation names (`extract`, `generate`),
language codes, model identifiers, hints — all are equal under the
protocol.

---

## 3. Parsing and Normalization

Cap URN processing distinguishes three forms:

| Form | Description |
|------|-------------|
| **Surface syntax** | What users may write (may omit `in`/`out`) |
| **Canonical form** | Normalized representation (always has `in`/`out`) |
| **Validation target** | Post-normalization structure that rules check |

### 3.1 Surface Syntax (Accepted Input)

Users may omit direction tags or write the trivial wildcard
explicitly. These are all valid surface syntax:
```
cap:test
cap:in=media:;out=media:;test
cap:in=*;test;out=*
```

### 3.2 Normalization to Canonical Form

Parsing produces a unique canonical representative per cap. Two
rules govern the directional axes:

1. Missing or wildcard direction tags resolve to `media:` internally.
2. When `in` resolves to the top media URN (`media:`), the segment
   is **omitted** in canonical form. Same for `out`. The internal
   value is still `media:`; the rendered form just doesn't show it.

| Surface Syntax | Canonical Form |
|----------------|----------------|
| `cap:test` | `cap:test` |
| `cap:in=media:;test;out=media:` | `cap:test` |
| `cap:in=*;test;out=*` | `cap:test` |
| `cap:in=media:pdf;extract;out=media:textable` | `cap:extract;in=media:pdf;out=media:textable` |
| `cap:` | `cap:` |
| `cap:in=media:;out=media:` | `cap:` |

The value `*` in direction tags expands to `media:`:
```
in=*  →  in=media:
out=* →  out=media:
```

This ensures `media:` is the unique top of the directional order, and
the canonical form is byte-equal across every way of writing it.

### 3.3 Validation Target

Validation rules (CU1, CU2 in [10-VALIDATION-RULES](./10-VALIDATION-RULES.md)) apply to the **canonical form**, not surface syntax. After normalization:
- `in` and `out` are always present
- Their values are valid Media URNs

### 3.4 Quoting

Direction spec values containing `;` must be quoted:
```
cap:in="media:pdf;bytes";extract;out="media:object"
```

Without quotes, `media:pdf;bytes` would parse incorrectly.

---

## 4. Cap Kinds

The (i, o, y) triple admits a five-way classification by inspecting
the directional axes. The classification is **logical only** — the
dispatch protocol does not branch on kind. Tools, UIs, planners, and
human readers use it to talk about what a cap *does* without
re-deriving the rules each time.

Two anchor types make the taxonomy fall out:

- **`media:`** is the **top type** — the universal wildcard over the
  media URN order. Every other media URN `conforms_to` this one. A
  side typed as `media:` reads as "any A": there is no constraint on
  what data flows there.
- **`media:void`** is the **unit type** — the nullary value. A side
  typed as `media:void` reads as "()": no meaningful data flows
  there. It is *not* "invalid" or "absent"; it is the type with
  exactly one value.

### 4.1 The Five Kinds

| Kind        | `i`           | `o`           | `y`           | Reads as     |
|-------------|---------------|---------------|---------------|--------------|
| `Identity`  | `media:`      | `media:`      | empty         | `A → A`      |
| `Source`    | `media:void`  | not `void`    | any           | `() → B`     |
| `Sink`      | not `void`    | `media:void`  | any           | `A → ()`     |
| `Effect`    | `media:void`  | `media:void`  | any           | `() → ()`    |
| `Transform` | anything else (at least one side non-void)   | `A → B`      |

Each implementation exposes this classification via a `kind()` method
on `CapUrn` (or its language-port equivalent), returning a `CapKind`
enum value.

### 4.2 Identity Is Fully Generic

`Identity` is the **fully generic** cap on every axis:

- input wide open (`media:`),
- output wide open (`media:`),
- no operation/metadata tags.

The canonical form is `cap:` and only `cap:`. Adding any tag — even
one with no special meaning — specifies *something* on the third
axis and demotes the morphism from `Identity` to a `Transform` whose
`in`/`out` happen to be the wildcards. So:

| URN              | Kind      | Reading                                |
|------------------|-----------|----------------------------------------|
| `cap:`           | Identity  | `A → A` for any `A`                    |
| `cap:passthrough`| Transform | "for the routing label *passthrough*, accept any input, produce any output" |

`Identity` is also the **top** of the Cap partial order: every other
cap is more specific. Specificity 0.

```rust
pub const CAP_IDENTITY: &str = "cap:";
```

Every capset **must** include the identity cap (see CU1 in
[10-VALIDATION-RULES](./10-VALIDATION-RULES.md)).

### 4.3 Source, Sink, Effect: void as Unit

`media:void` lets the `(i, o, y)` triple express caps that are not
data transformers in the conventional sense.

- A **Source** has `i = media:void` and `o ≠ media:void`. It produces
  a value with no meaningful input. Examples: warming a model
  (`cap:in=media:void;out=media:model-artifact;warm`), search-models,
  list-compatible-models, generators driven by configuration alone.
- A **Sink** has `i ≠ media:void` and `o = media:void`. It absorbs a
  value with no meaningful output. Examples: discard caps, log-to-
  telemetry, append-to-index.
- An **Effect** has both sides `media:void`. Reads as `() → ()`. A
  nullary side-effect cap: warm-cache, ping, health-check,
  initialize-index, sync-registry, log-heartbeat. Valid in the type
  theory; useful in practice for command-style operations whose
  whole purpose is the side effect.

In all three cases the `y` dimension may carry any tags. `media:void`
on a side is a directional decision; `y` continues to refine the
identity of the cap.

### 4.4 Transform: The Default

`Transform` is the catch-all: at least one side is a non-void media
URN, and the cap is not the bare identity. Transform covers the
overwhelming majority of caps in practice — the actual data
processors (extract, render, generate-text, embed, convert).

### 4.5 Why the Distinction Is Logical Only

Dispatch (the `accepts` / `conforms_to` predicates) operates on the
`(i, o, y)` triple uniformly. It does not consult `CapKind`. A
`Source` and a `Transform` whose `in` happens to specialize a
pattern's `media:void` are matched by the same rules; the kind is a
description of the result, not a routing dimension.

This separation matters because:

- The protocol stays simple (one matching rule, three axes).
- Tools and humans can still reason about caps in plain terms
  ("this is a Source — it doesn't take input").
- The kind cannot drift: it is always derivable from the URN. There
  is no separate field to keep in sync, and no flag a cartridge
  could set wrongly.

---

## 5. Dimension Semantics

The three axes of `(i, o, y)` correspond to three independent
questions. The kind taxonomy from §4 is what falls out when those
questions are answered.

### 5.1 Input Dimension (i)

`in` answers: *what data does this cap consume?*

| Value          | Meaning                                          |
|----------------|--------------------------------------------------|
| `media:pdf`    | "Requires a PDF."                                |
| `media:`       | "Accepts any input." (top — Identity / generic)  |
| `media:void`   | "Takes no data input." (unit — Source / Effect)  |

`media:` and `media:void` are not interchangeable: one says "anything
goes here," the other says "nothing flows here."

### 5.2 Output Dimension (o)

`out` answers: *what data does this cap produce?*

| Value          | Meaning                                          |
|----------------|--------------------------------------------------|
| `media:json`   | "Produces JSON."                                 |
| `media:`       | "May produce any output." (top — Identity / generic) |
| `media:void`   | "Produces no data." (unit — Sink / Effect)       |

### 5.3 Cap-Tags Dimension (y)

`y` answers: *what specifies, refines, or labels this cap beyond its
data signature?*

```
cap:...;extract;target=metadata
```

`y` is itself a Tagged URN (without prefix), with the same matching
semantics as any other Tagged URN. Tags in `y` are arbitrary — no
key has functional meaning to the protocol. They distinguish caps
with the same data signature (e.g. an `extract` cap and a `summarize`
cap can both have `media:pdf → media:textable` and remain distinct
because their `y` differs).

A non-empty `y` is also what distinguishes `cap:passthrough`
(Transform) from `cap:` (Identity), even though the directional
axes match.

---

## 6. Accessing Components

### 6.1 Extracting Dimensions

Given a Cap URN string, extract:

```rust
let cap = CapUrn::from_string("cap:extract;in=media:pdf;out=media:textable")?;

let input: &str = cap.in_spec();    // "media:pdf"
let output: &str = cap.out_spec();  // "media:textable"
let has_extract: bool = cap.has_marker_tag("extract"); // true
let kind: CapKind = cap.kind()?;    // CapKind::Transform
```

`kind()` derives the [CapKind](#4-cap-kinds) classification from the
parsed `(i, o, y)` triple. It returns an error only on internally
inconsistent state (which `CapUrn` construction prevents) — a hard
signal that something upstream is broken.

### 6.2 Component Types

| Component | Type             | Access            |
|-----------|------------------|-------------------|
| Input     | Media URN string | `in_spec()`       |
| Output    | Media URN string | `out_spec()`      |
| Cap-tags  | Key-value map    | `tags`, `tag(key)`|
| Kind      | `CapKind` enum   | `kind()`          |

---

## 7. Specificity

Cap URN specificity is defined in
[05-SPECIFICITY](./05-SPECIFICITY.md). All three axes are scored by
the same six-form per-tag ladder (`?x`:0, `x?=v`:1, `x` (=`x=*`):2,
`x!=v`:3, `x=v`:4, `!x`:5), but the axes are *weighted*:

```
spec_C(c) = 10_000 * spec_U(c.out)
          +    100 * spec_U(c.in)
          +          spec_U(c.y)
```

The lexicographic priority `(out, in, y)` reflects routing intent:
producing different things is the largest semantic difference between
two caps; consuming different things is next; descriptive y-axis
metadata is last. Two orders of magnitude separate each axis so
per-axis sums up to ~99 stay in their own digit slot, making the
integer both totally ordered and visually decodable (`40205` reads
as out=4, in=2, y=5).

Examples by kind, showing per-axis sums `(out, in, y)` and the
weighted total:

| URN                                              | Kind      | (out, in, y) | spec_C |
|--------------------------------------------------|-----------|:------------:|-------:|
| `cap:`                                           | Identity  | (0, 0, 0)    |      0 |
| `cap:extract`                                    | Effect    | (0, 0, 2)    |      2 |
| `cap:extract;in=media:pdf;out=media:textable`    | Transform | (2, 2, 2)    |  20202 |
| `cap:in=media:void;out=media:void;ping`          | Effect    | (2, 2, 2)    |  20202 |
| `cap:extract;target=metadata`                    | Effect    | (0, 0, 6)    |      6 |

Identity is uniquely at specificity 0 (top of the order). Adding any
tag whose form scores above 0 — directional or otherwise — moves a
cap below identity in the specialization order.

---

## 8. Partial Order Structure

Cap URNs form a partial order (specialization order) in the product
space. `cap:` (Identity) is the top:

```
                            cap:                         (top, Identity)
                             |
                        cap:extract                       (Transform)
                       /            \
       cap:extract;in=media:pdf       cap:extract;out=media:textable
                       \            /
            cap:extract;in=media:pdf;out=media:textable
                             |
   cap:extract;in=media:pdf;out=media:textable;target=metadata     (more specific)
```

The ordering follows from the dispatch relation (see
[07-DISPATCH](./07-DISPATCH.md)). Note that the kind can change as
you move down the lattice — `cap:` is Identity, every refinement of
it is a Transform (or Source/Sink/Effect once a side becomes void).

---

## 9. Relationship to Media URNs

### 9.1 Direction Values Are Media URNs

The `in` and `out` tag values are themselves Media URNs:

```
cap:in="media:pdf;bytes";out="media:object"
        ↑                     ↑
    Media URN             Media URN
```

### 9.2 Matching Uses Media URN Semantics

When matching direction specs, use Media URN matching:

```rust
let provider_in = MediaUrn::from_string("media:bytes")?;
let request_in = MediaUrn::from_string("media:pdf;bytes")?;

// For dispatch: request_in must conform to provider_in
request_in.conforms_to(&provider_in)  // true
```

---

## 10. Validation Rules

Cap URNs must satisfy (from [10-VALIDATION-RULES](./10-VALIDATION-RULES.md)):

- **CU1**: Must have `in` and `out` tags (enforced via normalization)
- **CU2**: Direction values must be valid Media URNs or `*`

---

## 11. Common Patterns by Kind

This section walks the five kinds with concrete, idiomatic examples.

### 11.1 Identity

```
cap:
```

The identity morphism. Fully generic on every axis. Required in
all capsets.

### 11.2 Transform — typed data processor

```
cap:extract;in=media:pdf;out=media:textable
cap:generate;constrained;in=media:textable;language=en;out=media:json
cap:render-page-image;in=media:pdf;out=media:image
```

The bread and butter: real data flows in, real data flows out, the
`y` dimension labels the operation and any modifiers.

### 11.3 Source — generator

```
cap:in=media:void;out=media:model-artifact;warm
cap:in=media:void;out=media:model-list;list-compatible-models
cap:in=media:void;out=media:embeddings-dim;numeric;target=embeddings-dim
```

`media:void` on the input side: the cap produces a value driven by
its `y` (configuration tags, args, peer state) rather than a piped
input.

### 11.4 Sink — consumer

```
cap:discard;in=media:;out=media:void
cap:in=media:json;log;out=media:void
```

`media:void` on the output side: the cap absorbs a value but
contributes no data to the downstream flow. Useful as a graph
terminator.

### 11.5 Effect — nullary side-effect

```
cap:in=media:void;out=media:void;ping
cap:in=media:void;out=media:void;warm-cache
cap:in=media:void;out=media:void;health-check
```

Both sides `media:void`. The whole point of the cap is the side
effect: the protocol carries no data either way, only the
invocation. Read as `() → ()`.

---

## 12. Summary

| Concept            | Definition                                                  |
|--------------------|-------------------------------------------------------------|
| Structure          | `C = U × U × U` (product of Tagged URN domain)              |
| Components         | `(in, out, y)`                                              |
| Top type           | `media:` — the universal wildcard for either direction      |
| Unit type          | `media:void` — nullary value (no data flows on this side)   |
| Identity           | `cap:` (canonical) — fully generic on every axis             |
| Direction defaults | Missing or `*` → `media:`; canonical drops `in`/`out` when both are `media:` and `y` is empty |
| Functional axes    | Only `in` and `out` participate in dispatch; `y` is arbitrary metadata |
| Kinds              | Identity, Source, Sink, Effect, Transform — derived from `(i, o)` and "is `y` empty?"; logical-only |

Cap URNs extend Tagged URNs with three-dimensional structure. The
dispatch relation (next document) defines how these dimensions
interact for routing. Once dispatch is in place, multiple Cap URNs
can be wired into a data-flow graph and serialized via
[09-MACHINE-NOTATION](./09-MACHINE-NOTATION.md).
