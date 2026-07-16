---
title: "Dispatch"
layout: doc
permalink: /07-dispatch/
---
# Dispatch

## 1. The Central Question

Given:
- A **candidate** `p` (a registered capability)
- A **request** `r` (what the caller wants)

The dispatch question is:

> Can candidate `p` legally handle request `r`?

This is answered by the **dispatch predicate**.

The predicate is **kind-agnostic**. The
[CapKind](/docs/06-cap-urn-structure#4-cap-kinds) classification
(Identity / Source / Sink / Effect / Transform) is a logical taxonomy
derived from the URN; it does not appear in the dispatch rule.
Whether a candidate is a Source matching a request whose `in` happens
to be `media:void`, or a Transform matching a request whose `in` is a
concrete type, is the same matching rule applied to the same four
structural coordinates. Dispatch is one rule; kind is a description of the result.

---

## 2. The Dispatch Predicate

### 2.1 Definition

Let:
- `p = (i_p, o_p, y_p, e_p)` — candidate
- `r = (i_r, o_r, y_r, e_r)` — request

Then:

```
Dispatch(p, r)  ⟺  (i_r = ⊤ ∨ i_r ⪯ i_p)  ∧  (o_r = ⊤ ∨ o_p ⪯ o_r)  ∧  (e_r = ? ∨ e_p = e_r)  ∧  y_r ⪯ y_p
```

Where `⊤ = media:` (the identity/top of the media partial order). A request dimension
set to `⊤` is **unconstrained** — the axis is vacuously true.

Note: candidate wildcards need no special case. `i_p = ⊤` passes because `∀x, x ⪯ ⊤`.
`o_p = ⊤` correctly fails for specific `o_r` because `⊤ ⪯ o_r` is false (top does not
conform to a more specific type).

### 2.2 The Four Conjuncts

| Axis | Condition | Variance | Meaning |
|------|-----------|----------|---------|
| Input | i_r = ⊤ ∨ i_r ⪯ i_p | Contravariant | Request unconstrained, or input conforms to candidate |
| Output | o_r = ⊤ ∨ o_p ⪯ o_r | Covariant | Request unconstrained, or candidate output conforms |
| Effect | e_r = ? ∨ e_p = e_r | Exact unless explicit wildcard | Candidate satisfies requested runtime effect semantics |
| Cap-tags | y_r ⪯ y_p | Invariant/Refinement | Candidate satisfies request's constraints |

---

## 3. Variance Interpretation

### 3.1 Input Axis (Contravariant)

```
i_r ⪯ i_p
```

**Meaning**: The candidate may accept MORE input types than the request specifies.

**Type-theoretic**: Function parameter types are contravariant.

**Example**:
```
Request:  in="media:bytes;ext=pdf"     (specific)
Candidate: in="media:bytes"         (more general)

i_r = media:bytes;ext=pdf
i_p = media:bytes

i_r ⪯ i_p? → Does pdf;bytes conform to bytes?
           → Yes, pdf;bytes is more specific than bytes
           → PASS ✓
```

A candidate accepting `media:bytes` can handle a request sending `media:bytes;ext=pdf`.

### 3.2 Output Axis (Covariant)

```
o_p ⪯ o_r
```

**Meaning**: The candidate must produce AT LEAST as specific output as the request requires.

**Type-theoretic**: Function return types are covariant.

**Example**:
```
Request:  out="media:record"                    (general requirement)
Candidate: out="media:fmt=json;record"     (more specific guarantee)

o_p = media:fmt=json;record
o_r = media:record

o_p ⪯ o_r? → Does fmt=json;record conform to record?
           → Yes, fmt=json;record is more specific than record
           → PASS ✓
```

A candidate guaranteeing `media:fmt=json;record` satisfies a request needing `media:record`.

### 3.3 Cap-Tags Axis (Invariant for Explicit, Wildcard for Omitted)

```
y_r ⪯ y_p
```

**Meaning**: The candidate must satisfy all explicit request constraints and may refine omitted ones.

**Example**:
```
Request:  op=extract                   (requires extract operation)
Candidate: extract;target=metadata   (provides extract with refinement)

y_r = {op: "extract"}
y_p = {op: "extract", target: "metadata"}

y_r ⪯ y_p? → Does request conform to candidate?
           → Request has op=extract, candidate has op=extract → match
           → Request omits target, candidate has target=metadata → OK (refinement)
           → PASS ✓
```

---

## 4. Dispatch Is NOT Symmetric

**Critical**: `Dispatch(p, r)` does NOT imply `Dispatch(r, p)`.

### 4.1 The Rule

The input condition `i_r ⪯ i_p` means:
- Request's input must be **at least as specific** as candidate's input
- Equivalently: Candidate's accepted input must **subsume** request's input

### 4.2 Why Asymmetry Matters

When request has `in=media:model-spec`:
- Request says "I will send model-spec"
- Candidate with `in=media:bytes` says "I accept any bytes"
- Can candidate handle this? **YES** — model-spec conforms to bytes
- `media:model-spec ⪯ media:bytes` is TRUE

When request has `in=media:bytes`:
- Request says "I will send bytes"
- Candidate with `in=media:model-spec` says "I only accept model-spec"
- Can candidate handle this? **NO** — bytes does not conform to model-spec
- `media:bytes ⪯ media:model-spec` is FALSE

### 4.3 Wildcard Handling

`media:` is the identity (top of the partial order). As a dimension value in dispatch, it means
"unconstrained" — the axis imposes no restriction and is vacuously true.

For dispatch validity with wildcards:

| Request Input | Candidate Input | Dispatch? | Reason |
|---------------|----------------|-----------|--------|
| `media:` | `media:` | ✓ | Both unconstrained |
| `media:` | `media:ext=pdf` | ✓ | Request unconstrained |
| `media:ext=pdf` | `media:` | ✓ | Candidate accepts any |
| `media:ext=pdf` | `media:bytes` | ✓ | pdf conforms to bytes |
| `media:ext=pdf` | `media:image` | ✗ | pdf does not conform to image |

---

## 5. Axis-by-Axis Rules

### 5.1 Input Axis

| Request In | Candidate In | Dispatchable? | Reason |
|------------|-------------|---------------|--------|
| `media:` (any) | any | ✓ | Request unconstrained |
| specific | `media:` (any) | ✓ | Candidate accepts any |
| specific | same | ✓ | Exact match |
| more specific | less specific | ✓ | Candidate accepts broader class |
| less specific | more specific | ✗ | Request might send unsupported |
| incomparable | incomparable | ✗ | Different type families |

### 5.2 Output Axis

| Candidate Out | Request Out | Dispatchable? | Reason |
|--------------|-------------|---------------|--------|
| any | `media:` (any) | ✓ | Request unconstrained |
| `media:` (any) | specific | ✗ | Candidate can't guarantee required |
| same | same | ✓ | Exact match |
| more specific | less specific | ✓ | Candidate exceeds requirement |
| less specific | more specific | ✗ | Candidate may not meet requirement |
| incomparable | incomparable | ✗ | Different type families |

### 5.3 Cap-Tags Axis

| Request Tag | Candidate Tag | Dispatchable? | Reason |
|-------------|--------------|---------------|--------|
| missing | missing | ✓ | No constraint |
| missing | K=v | ✓ | Candidate refines |
| K=v | K=v | ✓ | Exact match |
| K=v | K=w (w≠v) | ✗ | Contradiction |
| K=v | missing | ✗ | Candidate lacks required |
| K=* | K=v | ✓ | Candidate has a value |
| K=* | missing | ✗ | Candidate lacks required |

---

## 6. Examples

### 6.1 Generic Request, Specific Candidate

```
Request:  cap:in=media:;download-model;out=media:
Candidate: cap:in="media:model-spec";download-model;out="media:download-result"

Input:  i_r=media: (⊤), i_p=media:model-spec
        Request unconstrained → PASS ✓

Output: o_p=media:download-result, o_r=media: (⊤)
        Request unconstrained → PASS ✓

Tags:   y_r={op:download-model}, y_p={op:download-model}
        Candidate has required op → PASS ✓

Result: DISPATCHABLE ✓
```

### 6.2 Specific Request, Generic Candidate (Fallback)

```
Request:  cap:in="media:ext=pdf";extract;out="media:record"
Candidate: cap:in="media:bytes";extract;out="media:"

Input:  i_r=media:ext=pdf, i_p=media:bytes
        pdf ⪯ bytes? Yes → PASS ✓

Output: o_p=media:, o_r=media:record
        media: ⪯ media:record? No, top is NOT more specific
        → FAIL ✗

Result: NOT DISPATCHABLE
```

### 6.3 Incompatible Types

```
Request:  cap:in="media:ext=pdf";convert;out="media:ext=html"
Candidate: cap:in="media:image";convert;out="media:enc=utf-8"

Input:  i_r=media:ext=pdf, i_p=media:image
        pdf ⪯ image? No, different families → FAIL ✗

Result: NOT DISPATCHABLE (fails at first axis)
```

---

## 7. Properties

### 7.1 Reflexivity

```
∀c ∈ C, Dispatch(c, c)
```

Any capability can handle itself.

**Proof**: For c = (i, o, y, e):
- i ⪯ i (reflexivity of ⪯)
- o ⪯ o (reflexivity of ⪯)
- e = e
- y ⪯ y (reflexivity of ⪯)
- All four hold, so Dispatch(c, c) ✓

### 7.2 Transitivity

```
Dispatch(a, b) ∧ Dispatch(b, c) ⟹ Dispatch(a, c)
```

If a can handle b's requests, and b can handle c's requests, then a can handle c's requests.

**Proof**: By transitivity of ⪯ on the `in`, `out`, and `y` coordinates, plus equality transitivity on `effect`.

### 7.3 NOT Symmetric

```
Dispatch(p, r) ⟹̸ Dispatch(r, p)
```

A specific candidate can dispatch a generic request, but not vice versa.

### 7.4 Monotonicity

If candidate `p'` refines `p`:
- Same or more general input (i_p ⪯ i_p')
- Same or more specific output (o_p' ⪯ o_p)
- Same or more specific y-tags (y_p ⪯ y_p')

Then:
```
Dispatch(p, r) ⟹ Dispatch(p', r)
```

Refinement preserves dispatchability.

---

## 8. Implementation

### 8.1 Method Signature

```rust
impl CapUrn {
    pub fn is_dispatchable(&self, request: &CapUrn) -> bool;
}
```

Usage:
```rust
if candidate.is_dispatchable(&request) {
    // candidate can handle request
}
```

### 8.2 Pseudocode

```rust
fn is_dispatchable(&self, request: &CapUrn) -> bool {
    // Input axis (contravariant)
    // media: is unconstrained — vacuously true on either side
    if request.in_urn != "media:" && self.in_urn != "media:" {
        let req_in = MediaUrn::from_string(&request.in_urn);
        let prov_in = MediaUrn::from_string(&self.in_urn);
        if !req_in.conforms_to(&prov_in) {
            return false;
        }
    }

    // Output axis (covariant)
    // Request media: = unconstrained (accept anything) → pass
    // Candidate media: = no guarantee → fail when request is specific
    if request.out_urn == "media:" {
        // Request unconstrained — pass
    } else if self.out_urn == "media:" {
        return false; // Candidate can't guarantee specific output
    } else {
        let prov_out = MediaUrn::from_string(&self.out_urn);
        let req_out = MediaUrn::from_string(&request.out_urn);
        if !prov_out.conforms_to(&req_out) {
            return false;
        }
    }

    // Effect axis: exact match unless the request explicitly uses ?effect
    if request.effect != "?" && self.effect != request.effect {
        return false;
    }

    // Cap-tags axis: candidate must satisfy request constraints
    if !self.cap_tags_dispatchable(request) {
        return false;
    }

    true
}
```

---

## 9. Common Mistakes

### 9.1 Using `accepts` for Dispatch

**Wrong**:
```rust
if candidate.accepts(&request) { /* dispatch */ }
```

This ignores the mixed-variance nature of Cap URNs.

### 9.2 Using `conforms_to` for Dispatch

**Wrong**:
```rust
if candidate.conforms_to(&request) { /* dispatch */ }
```

This also ignores mixed variance.

### 9.3 Checking Only One Axis

**Wrong**:
```rust
if candidate.op == request.op { /* dispatch */ }
```

All four structural coordinates must be checked.

---

## 10. Resolution vs. dispatch: which predicate?

Dispatch (`is_dispatchable`, this document) is not the only cap-matching
question the system asks. There are two, and they use two different predicates
on purpose:

| Question | Predicate | Symmetry | Used for |
|---|---|---|---|
| "Can candidate `p` *handle* request `r`?" | `p.is_dispatchable(r)` | directional | routing, planning, and "find **anything** that would match" |
| "Is declared cap `d` *the same cap* as resolved cap `c`?" | `d.is_equivalent(c)` | symmetric | **alias/cap resolution** — finding the cartridge that implements a specific cap |

### Why alias resolution uses `is_equivalent`, not dispatch

When you run `capdag <alias> …`, the alias is resolved in the fabric registry to
**exactly one concrete cap URN** — a fully-specified point in the lattice, not a
pattern. The next step is to find the cartridge that *implements that cap*. That
is a **resolution** question ("which cartridge declares *this* cap?"), not a
**dispatch** question ("which cartridge could *handle* this?").

The same CLI also exposes inspection and visualization surfaces around this
resolution: `capdag resolve <alias> [--no-cache]` prints a cap's resolved
definition (with `--no-cache` bypassing a version-keyed fabric cache that a
staging re-publish can leave stale), `capdag cache clear|refresh` invalidates or
renews that cache, and `capdag dag-viz <alias> --mermaid|--dot` renders the full
MachinePlan (ForEach/Collect/Merge/Split/InputSlot/Output nodes and typed edges).

We deliberately match with the **symmetric** `is_equivalent` (each side accepts
the other — identical lattice position), because:

- **Determinism / least surprise.** The alias names one specific cap. The user
  gets a cartridge that provides *that* cap, never a different, more‑general cap
  that merely *could* serve the request. `is_dispatchable` is directional and
  would let a cartridge declaring `cap:disbind;in="media:ext=pdf";out="media:"`
  (any output) stand in for a request for
  `cap:disbind;…;out="media:…;page;plain-text"` — a silent substitution of a
  different behavior than the alias named.
- **No accidental widening.** Resolution must not "fall back" to a looser
  candidate; if nothing implements the exact resolved cap, that is a real
  "no candidate" answer the caller needs to see (and fix by publishing the right
  cartridge), not something to paper over by dispatching to a near‑match.

Equivalence is decided on the parsed in/out/effect coordinates, **never** by
string equality — two semantically identical cap URNs can serialize differently
(tag order, the arbitrary `op` marker), so matching walks the parsed predicate,
and resolution never prefilters through a string-keyed index (which would drop
equivalent-but-differently-serialized candidates before equivalence is tested).

### Where each is used (keep them consistent)

- **Resolution (`is_equivalent`)** — `CartridgeRepo::get_suggestions_for_cap`
  and the dev-cartridge lookup in `CartridgeManager::find_cartridge_binary`.
  These two are the *same* run-path question and MUST use the same predicate.
- **Find-anything / dispatch (`is_dispatchable`)** — `get_cartridges_by_cap`
  (enumerate every capable cartridge) and the planner/router dispatch sites.

Rule of thumb: **resolving a name → `is_equivalent`; asking "who can handle
this?" → `is_dispatchable`.** Never mix them within one question.

### Abstract caps: resolve the umbrella, then dispatch to a concrete cap

An **abstract cap** is a generic-input dispatch umbrella — e.g.
`cap:disbind;out="media:enc=utf-8;ext=txt;page;plain-text"` (input `media:`,
i.e. any) or `cap:convert-image` (input and output both `media:`). It is a valid
alias target but is **never backed by a cartridge** and is **excluded from the
runnable cap graph** (`LiveMachinePlanGraph::add_cap` skips it), so it can never
appear as an edge the wizard or planner would offer and then fail to execute.

Running `capdag <abstract-alias> <file>` uses **both** predicates, in order:

1. **Resolve** the alias to the abstract cap URN — `is_equivalent` (which cap
   does this name mean?).
2. **Dispatch** to a concrete cap — `is_dispatchable`. The CLI detects the input
   file's media type, builds the request = the abstract cap with its input
   specialized to that media (and its output specialized to `--to <target>` when
   given), and asks `FabricRegistry::narrow_abstract_cap`: which **concrete**
   (non-abstract) cap URN `is_dispatchable` for that request? Exactly one → run
   it; zero → "no handler for this input"; more than one → ambiguous (the caller
   disambiguates with `--to`, or by naming the concrete alias).

Every abstract cap must have ≥1 concrete specialization in the same snapshot
(enforced at publish, `fabric/src/fabric.js` `validateAbstractCoverage`), so the
narrowing can always reach a real cartridge. This is the same two-question split
as an ordinary alias — `is_equivalent` for the name, `is_dispatchable` for "who
can handle this input?" — with the input-media detection supplying the request.

---

## 11. Summary

The dispatch predicate is:

```
Dispatch(p, r)  ⟺  (i_r = ⊤ ∨ i_r ⪯ i_p)  ∧  (o_r = ⊤ ∨ o_p ⪯ o_r)  ∧  (e_r = ? ∨ e_p = e_r)  ∧  y_r ⪯ y_p
```

Where `⊤ = media:` (unconstrained).

| Property | Value |
|----------|-------|
| Input variance | Contravariant |
| Output variance | Covariant |
| Effect variance | Exact unless explicit wildcard |
| Cap-tags variance | Invariant + Refinement |
| Symmetric? | NO |
| Reflexive? | YES |
| Transitive? | YES |

This is the **primary predicate for routing**. Ranking (next document) applies only after dispatch validity is established.
