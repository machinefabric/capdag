# Media URNs

## 1. Structure

A Media URN has the form:

```
media:<type>[;tag=value]...
```

Examples:
```
media:                          # Identity (any media)
media:pdf                       # PDF type
media:pdf;bytes                 # PDF with bytes marker
media:textable;form=scalar      # String type
media:image;subtype=png;visual  # PNG image
```

---

## 2. Top and Unit Types

The media URN order has two distinguished anchors. They are **not**
interchangeable: confusing them flips the meaning of every cap that
uses them.

### 2.1 `media:` — the Top Type

```
media:
```

The bare prefix with no tags. Reads as "any data type."

- Has no tags.
- Every other media URN `conforms_to` it.
- Specificity 0.
- The **top** of the media partial order.

```rust
pub const MEDIA_IDENTITY: &str = "media:";
```

```
∀m ∈ MediaUrn, m ⪯ media:     (every media URN conforms to top)
media: ⪯ media:               (reflexive)
```

A cap with `in=media:` says "I accept any input." A cap with
`out=media:` says "I may produce any output." Used on both sides of
a cap with no other tags, the cap is the [identity morphism](/docs/06-cap-urn-structure#4-cap-kinds)
of the category.

### 2.2 `media:void` — the Unit Type

```
media:void
```

Reads as "no data" — the nullary value, the type with exactly one
inhabitant.

- Has the `void` marker tag.
- Distinct from `media:`. Top means "any A flows here"; unit means
  "() flows here, no meaningful payload."

```rust
pub const MEDIA_VOID: &str = "media:void";
```

A cap with `in=media:void` does not consume data — it is driven
entirely by its non-directional tags and any peer state. A cap with
`out=media:void` produces no data; it exists for the side effect.
Caps with `media:void` on both sides are pure side-effect commands
(see [CapKind](./06-CAP-URN-STRUCTURE.md#4-cap-kinds): Source,
Sink, Effect).

#### Atomicity

`media:void` is **atomic**. The parser rejects any media URN that
combines the `void` marker tag with any other tag:

```
media:void                ✓
media:void;text           ✗  (parse error)
media:void;pdf            ✗  (parse error)
media:void;reason=warmup  ✗  (parse error)
media:void;heartbeat      ✗  (parse error)
```

There is no lattice underneath the unit. Permitting refinements
would manufacture a fake taxonomy of unit values
(`media:void;warmup` vs `media:void;heartbeat` etc.) and dispatch
semantics would silently fork: are these different units? different
effects? different commands? Refusing the syntax forecloses the
question.

When a cap needs to express *why* or *how* it uses void, that
information goes on the **cap URN's non-directional axis** (or in
cap args), never as a refinement of the media URN:

```
✓ cap:in=media:void;out=media:void;warmup
✓ cap:in=media:void;out=media:void;heartbeat
✓ cap:in=media:void;out=media:image;generate;target=thumbnail

✗ cap:in=media:void;reason=warmup;out=media:void
✗ cap:in=media:void;text;out=media:textable
```

Each of the first three describes a distinct morphism (different
operation tags). The last two try to pack the same distinction into
the unit type itself; the parser rejects them at the media-URN
layer before the cap URN ever forms.

This rule is enforced at the deepest layer — every `MediaUrn`
constructor and `from_string` parse path returns a parse error on
violation:

| Port      | Error                                |
|-----------|--------------------------------------|
| Rust      | `MediaUrnError::VoidNotAtomic`       |
| Go        | `MediaUrnError{Code: ErrorMediaVoidNotAtomic}` |
| Python    | `MediaUrnError("media:void is atomic …")` |
| Swift/ObjC| `CSMediaUrnErrorVoidNotAtomic`       |
| JS        | `MediaUrnError(VOID_NOT_ATOMIC, …)`  |

Cross-language parity is pinned by `test1810`.

### 2.3 Top vs Unit at a Glance

| Side type     | Reads as           | Used for                       |
|---------------|--------------------|--------------------------------|
| `media:`      | "any A"            | wildcards, generic passthrough |
| `media:void`  | "()"               | sources, sinks, effects        |
| concrete      | a specific type    | normal data flow               |

`media:` and `media:void` look superficially similar (both are
"unspecific" in some sense) but they sit at opposite ends of the
type lattice. `media:` is the **maximum** of the order; `media:void`
is a leaf carrying the nullary value.

---

## 3. Coercion Tags

Media URNs use **coercion tags** to declare type capabilities. These enable polymorphic matching.

### 3.1 Standard Coercion Tags

| Tag | Meaning | Examples |
|-----|---------|----------|
| `textable` | Can be represented as UTF-8 text | strings, numbers, booleans, JSON |
| `binary` | Raw bytes representation | images, PDFs, audio |
| `numeric` | Supports numeric operations | integers, floats |
| `scalar` | Single atomic value | primitives (not arrays/objects) |
| `sequence` | Ordered collection | arrays |
| `map` | Key-value structure | objects |
| `visual` | Has visual rendering | images, PDFs |

### 3.2 How Coercion Works

A capability requiring `media:textable` matches ANY type with the `textable` tag:

```
cap:in="media:textable";prompt;out="media:textable;form=map"
```

This matches:
- `media:textable;form=scalar` (string)
- `media:integer` (if it has textable)
- `media:bool;textable;form=scalar` (boolean)
- `media:textable;form=map` (object via JSON.stringify)

### 3.3 Coercion Rules

| Source Type | Can Coerce To | Method |
|-------------|---------------|--------|
| integer, number | textable | `.toString()` |
| boolean | textable | `"true"` / `"false"` |
| object, array | textable | JSON stringify |
| string | textable | Direct (already text) |
| image, PDF, audio | textable | **NO** (requires explicit conversion cap) |

---

## 4. Form Tags

The `form` tag specifies structural shape:

| Value | Meaning |
|-------|---------|
| `form=scalar` | Single value |
| `form=list` | Array/sequence |
| `form=map` | Object/dictionary |

### 4.1 Examples

```
media:textable;form=scalar         # String
media:textable;form=list           # Array of strings
media:textable;form=map            # JSON object
media:integer;textable;form=list   # Array of integers
```

---

## 5. Common Media Types

### 5.1 Primitives

| Media URN | Constant | Description |
|-----------|----------|-------------|
| `media:` | `MEDIA_IDENTITY` | **Top** — any data type (universal wildcard) |
| `media:void` | `MEDIA_VOID` | **Unit** — the nullary value (no data flows here) |
| `media:textable;form=scalar` | `MEDIA_STRING` | UTF-8 string |
| `media:integer` | `MEDIA_INTEGER` | Integer |
| `media:textable;numeric;form=scalar` | `MEDIA_NUMBER` | Float |
| `media:bool;textable;form=scalar` | `MEDIA_BOOLEAN` | Boolean |
| `media:textable;form=map` | `MEDIA_OBJECT` | JSON object |

### 5.2 Arrays

| Media URN | Constant | Description |
|-----------|----------|-------------|
| `media:textable;form=list` | `MEDIA_STRING_ARRAY` | String array |
| `media:integer;textable;form=list` | `MEDIA_INTEGER_ARRAY` | Integer array |
| `media:textable;numeric;form=list` | `MEDIA_NUMBER_ARRAY` | Number array |
| `media:bool;textable;form=list` | `MEDIA_BOOLEAN_ARRAY` | Boolean array |

### 5.3 Visual Types

| Media URN | Description |
|-----------|-------------|
| `media:image;subtype=png;visual` | PNG image |
| `media:image;subtype=jpeg;visual` | JPEG image |
| `media:application;subtype=pdf;visual` | PDF document |

---

## 6. Matching Semantics

Media URN matching follows Tagged URN semantics from [01-TAGGED-URN-DOMAIN](./03-TAGGED-URN-DOMAIN.md).

### 6.1 Pattern Matching

```
Pattern:  media:bytes
Instance: media:pdf;bytes

Does instance have all tags pattern requires?
- Pattern requires: bytes=*
- Instance has: pdf=*, bytes=*
- bytes present? Yes → MATCH ✓
```

### 6.2 Specificity

More tags = more specific:

```
spec(media:) = 0
spec(media:bytes) = 2           # bytes=* is must-have-any
spec(media:pdf;bytes) = 4       # two must-have-any tags
spec(media:pdf;v=2.0) = 5       # must-have-any + exact value
```

### 6.3 Conformance

```
media:pdf;bytes ⪯ media:bytes   (pdf;bytes conforms to bytes)
media:bytes ⪯ media:            (bytes conforms to identity)
media:pdf ⪯ media:image         ✗ (not on same chain)
```

---

## 7. Direction Specs in Cap URNs

When used as `in` or `out` values in Cap URNs:

### 7.1 Quoting

Media URNs containing `;` must be quoted:

```
cap:in="media:pdf;bytes";extract;out="media:object"
```

### 7.2 Identity Expansion

`in=*` and `out=*` expand to `media:`:

```
cap:in=*;convert;out=*
→ cap:in=media:;convert;out=media:
```

### 7.3 Dispatch

For dispatch (see [05-DISPATCH](./07-DISPATCH.md)):

- **Input**: Request input must conform to provider input (contravariant)
- **Output**: Provider output must conform to request output (covariant)

---

## 8. Type Detection

### 8.1 Helper Methods

```rust
let urn = MediaUrn::from_string("media:textable;form=scalar")?;

urn.is_text()    // true for string, text/*
urn.is_json()    // true for object, object-array
urn.is_binary()  // true for raw binary, images
urn.is_void()    // true iff the `void` marker tag is present (unit type)
urn.is_top()     // true iff the URN has no tags at all (top type)
```

`is_void` and `is_top` are the predicates the [CapKind](/docs/06-cap-urn-structure#4-cap-kinds)
classifier consults. Together they let any caller reason about
whether a media URN is a concrete type, the wildcard, or the unit
without parsing strings.

### 8.2 Tag Queries

```rust
urn.has_tag("textable")     // true
urn.tag_value("form")       // Some("scalar")
```

---

## 9. Adding New Types

When defining a new media type:

1. **Determine coercion tags**: What can this type be coerced to?
2. **Determine form**: scalar, list, or map?
3. **Add constant** if frequently used
4. **Document** in media catalog

### 9.1 Example: Custom Type

```rust
// A new type for structured logs
const MEDIA_LOG_ENTRY: &str = "media:log-entry;textable;form=map";

// Coercible to text (via JSON), structured as map
```

---

## 10. Partial Order Position

Media URNs form a partial order (specialization order). `media:` is
the unique top; `media:void` is a leaf, distinct from every concrete
type:

```
                    media:                          (top — any A)
                  /        \
          media:textable    media:void              (leaf — unit ())
            /         \
media:textable;form=scalar   media:textable;form=map
        |                            |
media:integer;textable       media:json;textable;form=map
```

More tags = lower in the order = more specific. `media:void` carries
exactly one tag (`void`) and so sits at specificity 2 (one
must-have-any tag, plus the prefix); it is not a refinement of any
concrete type.

---

## 11. Summary

| Concept | Definition |
|---------|------------|
| Structure | `media:<type>[;tag=value]...` |
| Top | `media:` — universal wildcard, max of the partial order, "any A" |
| Unit | `media:void` — nullary value, leaf of the partial order, "()" |
| Coercion tags | textable, binary, numeric, scalar, sequence, map, visual |
| Form tag | scalar, list, map |
| Matching | Tagged URN semantics (truth table) |
| Specificity | Sum of per-tag scores |

Media URNs describe data types. They are used:
- In Cap URN `in`/`out` specs (where the choice between `media:`,
  `media:void`, and a concrete type determines the cap's
  [kind](/docs/06-cap-urn-structure#4-cap-kinds))
- As argument identifiers
- For type matching in dispatch
