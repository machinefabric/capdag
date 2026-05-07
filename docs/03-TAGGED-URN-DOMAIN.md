# Tagged URN Domain

## 1. Definition

Let **U** be the set of all normalized Tagged URNs.

A Tagged URN has the structure:
```
prefix:key1=value1;key2=value2;...
```

Each element `u ∈ U` has:
- A **prefix** (e.g., `media`, `cap`)
- Zero or more **tags** as key-value pairs
- A well-defined **canonical representation**

---

## 2. Syntax

### 2.1 Grammar

```
tagged-urn  ::= prefix ":" tags?
prefix      ::= identifier
tags        ::= tag (";" tag)*
tag         ::= key ("=" value)?
key         ::= identifier
value       ::= unquoted-value | quoted-value
identifier  ::= [a-z][a-z0-9-]*
```

### 2.2 Examples

```
media:                           # Identity (no tags)
media:pdf;bytes                  # Two tags with implicit * values
cap:in=media:pdf;extract;out=media:object
cap:in="media:pdf;bytes";extract;out="media:object"
```

---

## 3. Normalization

All Tagged URNs are normalized on parse using these rules:

### 3.1 Case Normalization

| Component | Rule |
|-----------|------|
| Prefix | Lowercase |
| Keys | Lowercase |
| Unquoted values | Lowercase |
| Quoted values | **Preserve case** |

### 3.2 Value-less Tags

A tag without `=` is treated as having value `*`:
```
Input:  media:pdf;bytes
Parsed: media:pdf=*;bytes=*
```

When serializing, `*` values serialize back to value-less form:
```
Internal: {pdf: "*", bytes: "*"}
Output:   media:pdf;bytes
```

### 3.3 Tag Ordering

Tags are stored in a sorted map (BTreeMap) and serialized in **alphabetical key order**:
```
Input:  media:bytes;pdf
Output: media:bytes;pdf      # 'bytes' before 'pdf' alphabetically
```

### 3.4 Quoting Rules

A value requires quoting if it contains: `;`, `=`, `"`, `\`, space, or uppercase characters.

Inside quotes:
- `"` is escaped as `\"`
- `\` is escaped as `\\`
- All other characters are literal

---

## 4. The Six Canonical Forms

Tagged URNs encode constraints on each key using one of six canonical
forms. Several authored aliases collapse to the same canonical form
during parsing; serialization always emits the canonical
representative deterministically.

| Authored aliases             | Canonical | Stored value | Reading                             |
|------------------------------|-----------|--------------|-------------------------------------|
| `?x` ≡ `x?` ≡ `x=?`          | `?x`      | `"?"`        | no constraint                       |
| `?x=v` ≡ `x?=v` ≡ `x=?v`     | `x?=v`    | `"?=v"`      | absent OR (present and not v)       |
| `x` ≡ `x=*`                  | `x`       | `"*"`        | present with any value (must-have)  |
| `!x=v` ≡ `x!=v` ≡ `x=!v`     | `x!=v`    | `"!=v"`      | present and not v                   |
| `x=v`                        | `x=v`     | `"v"`        | present and exactly v               |
| `!x` ≡ `x!` ≡ `x=!`          | `!x`      | `"!"`        | absent (must-not-have)              |

### 4.1 Qualifier Position

The qualifier `?` or `!` may appear EITHER as a key prefix
(`?x`, `!x`, `?x=v`, `!x=v`) OR as an infix immediately before `=`
(`x?`, `x!`, `x?=v`, `x!=v`). The two notations are exact aliases.

### 4.2 Disallowed Combinations

Hard parse errors:

- **Mixed prefix and infix**: `?x?`, `?x?=v`, `!x!=v`, `!x?` — the qualifier appears twice on the same key.
- **Mixed `?` and `!`**: `?!x`, `!?x` — contradictory.
- **Qualifier with sigil value**: `?x=*`, `!x=*`, `?x=?`, `?x=!` — a qualifier and a sigil-only value would conflate the two semantics.
- **Empty value**: `x=` — exact value cannot be empty.
- **Bare `?` or `!` with nothing after**: `prefix:?`, `prefix:!` — qualifier requires a key.

### 4.3 Why Six Forms

The six forms partition into two semantic chains plus their endpoints:

- **Positive chain** (presence claims): `?x` (no constraint) → `x` (must-have-any) → `x=v` (must-have-exactly-v).
- **Negative chain** (absence/exclusion claims): `?x` (no constraint) → `x?=v` (absent or not v) → `x!=v` (present and not v) → `!x` (absent).

Both chains start at the same shared origin (`?x`, no constraint).
The chains diverge by the kind of claim being made: positive chains
*require* presence and progressively pin the value; negative chains
*forbid* values and progressively narrow the kind of forbidding.

### 4.4 Canonical of `x=*`

Bare `x` and `x=*` are the same form. The parser stores both with
value `"*"`; the serializer emits the bare form `x` (no `=*` suffix)
as canonical. This keeps marker-style writing (`extract`, `bytes`,
`textable`) consistent with explicit-value writing.

---

## 5. Constraint Truth Table

The full 6×6 cross-product (including the implicit "missing" form
for keys with no entry) is in [04-PREDICATES](./04-PREDICATES.md).
The headline rules:

1. **Pattern missing or `?x`**: always matches (no constraint).
2. **Pattern `!x`**: matches only if instance is missing, `?x`, `x?=v`, or `!x` (i.e. instance does not have the key with a value).
3. **Pattern `x`** (must-have-any): matches only if instance has the key with some value (`x`, `x!=v`, `x=v`, or instance-side `?x` defers to runtime).
4. **Pattern `x=v`** (exact): matches only if instance has the key with exactly `v` (or instance is `?x`/`x` deferring to runtime).
5. **Pattern `x?=v`** (absent or not v): matches if instance is absent, or has any value other than `v`.
6. **Pattern `x!=v`** (present and not v): matches if instance has the key with any value other than `v`.

### 5.1 Reading the Table

- **Row**: What the instance has for key K (one of the six forms or missing).
- **Column**: What the pattern requires for key K.
- **Cell**: ✓ = match, ✗ = no match. Some cells depend on whether the values overlap.

### 5.2 Examples

```
# Pattern: media:pdf      Instance: media:pdf;bytes
# Pattern has: pdf=*      Instance has: pdf=*, bytes=*
# For key 'pdf': Instance=*, Pattern=* → ✓
# For key 'bytes': Instance=*, Pattern=(missing) → ✓
# Result: MATCH

# Pattern: media:pdf;!audio    Instance: media:pdf;audio=mp3
# For key 'pdf': Instance=*, Pattern=* → ✓
# For key 'audio': Instance=mp3 (exact), Pattern=! → ✗
# Result: NO MATCH

# Pattern: media:pdf;v!=draft   Instance: media:pdf;v=final
# For key 'pdf': Instance=*, Pattern=* → ✓
# For key 'v': Instance=final, Pattern=!=draft → ✓ (final ≠ draft)
# Result: MATCH

# Pattern: media:pdf;v?=draft   Instance: media:pdf
# For key 'pdf': Instance=*, Pattern=* → ✓
# For key 'v': Instance=missing, Pattern=?=draft → ✓ (absence allowed)
# Result: MATCH
```

---

## 6. The Base Relation ⪯

Define a binary relation on Tagged URNs:

```
a ⪯ b   iff   a is at least as specific as b
```

Equivalently:
- `a` refines `b`
- `b` accepts `a`
- `a` is lower in the partial order than `b`

### 6.1 Operational Definition

`a ⪯ b` holds iff for every key K:
- If `b` has a constraint on K, then `a` satisfies that constraint per the truth table
- `a` may have additional keys that `b` doesn't mention

### 6.2 Properties

The relation ⪯ is:
- **Reflexive**: `∀u ∈ U, u ⪯ u`
- **Transitive**: `(a ⪯ b ∧ b ⪯ c) ⟹ a ⪯ c`
- **Antisymmetric (modulo normalization)**: `(a ⪯ b ∧ b ⪯ a) ⟹ a ≡ b`

This makes ⪯ a **partial order** on normalized URNs.

---

## 7. Identity Elements

### 7.1 Media Identity

The Media URN identity is:
```
media:
```

This URN:
- Has no tags
- Accepts any Media URN (top of the partial order)
- Has specificity 0

**Reflexivity**: `media: ⪯ media:` holds (identity accepts itself).

### 7.2 Tagged URN Identity

For any prefix P, the identity is:
```
P:
```

A URN with no tags accepts any URN with the same prefix.

---

## 8. Partial Order Structure

Tagged URNs with the same prefix form a **partial order** (specialization order):

```
                    media:                    (top - most generic)
                      |
              media:bytes
                /         \
       media:pdf          media:image
           |                   |
   media:pdf;v=2.0      media:image;png
           |
media:pdf;v=2.0;compressed              (more specific)
```

- **Top (⊤)**: The identity `prefix:` with no tags
- **Ordering**: More tags = more specific = lower in the order

Note: While this structure has lattice-like properties (bounded above by identity), we do not formally define join (⊔) or meet (⊓) operations. The system uses only the partial order relation ⪯.

---

## 9. Parse Errors

The following conditions produce parse errors:

| Condition | Error |
|-----------|-------|
| Missing prefix | `MissingPrefix` |
| Empty prefix | `EmptyPrefix` |
| Duplicate key | `DuplicateKey` |
| Numeric key | `NumericKey` |
| Empty value after `=` | `EmptyValue` |
| Unclosed quote | `UnclosedQuote` |
| Invalid escape sequence | `InvalidEscape` |

---

## 10. Summary

The Tagged URN domain U provides:

1. **Normalized representation** — Canonical string form via case/ordering rules
2. **Six-form constraint alphabet** — `?x`, `x?=v`, `x` (=`x=*`), `x!=v`, `x=v`, `!x`, plus the implicit "missing"
3. **Partial order ⪯** — Reflexive, transitive, antisymmetric relation
4. **Lattice structure** — Identity at top, specificity increasing downward
5. **Compositional matching** — Per-key matching via the 6×6 truth table

All higher-level constructs (predicates, Cap URNs, dispatch) build on this foundation.
