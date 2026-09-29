# Revision Agreement: a conformance specification for derived-consumer currency

**Status:** draft · **Version:** 0.1.0 · **Date:** 2026-09-29

A system that stores knowledge and serves it through derived copies cannot be
trusted unless every copy can say which revision it is serving, and the system
refuses to claim currency when they disagree.

This document specifies that property, its conformance levels, and how to test
for it. It is deliberately independent of storage engine, index type, and
language.

---

## 1. Motivation

Systems that answer questions from a corpus almost always have the same shape:

- a **source of truth** that holds the authoritative content, and
- one or more **derived consumers** built from it — a search index, a rendered
  projection, a cache, a replica, a mirror.

Derived consumers exist because the source is the wrong shape for reading fast.
They are copies, and copies lag.

The failure this specification addresses is not corruption and not an outage. It
is a derived consumer answering **correctly for a revision that is no longer
current**, with no indication to the caller that anything is behind. Every
component reports success. The answer is well-formed. It is simply out of date,
and the caller acts on it.

This failure mode is invisible to health checks that ask "is the service up?"
and to tests that ask "does the query return results?" It is only visible to a
check that asks "results *as of what*?"

**Design note.** Two mature traditions already solve adjacent problems, and
neither one solves this:

- **Cache invalidation** keeps derived state correct inside a process by keying
  it on a version sentinel. The sentinel is an implementation detail; it is
  motivated by performance, it is usually process-local, and it is not reported
  to the caller.
- **Transparency logs** make write history verifiable after the fact through
  append-only structures, signed checkpoints and consistency proofs. They prove
  history was not rewritten; they do not bind a *read* to the state that
  produced it.

Revision agreement sits between them: carry the version across the boundary to
the reader, at the moment the reader needs it, and act on disagreement.

---

## 2. Terminology

The key words MUST, MUST NOT, SHOULD and MAY are to be interpreted as in
RFC 2119.

- **Revision** — an identifier that uniquely names one immutable state of the
  source of truth. A content-addressed hash (for example a Git commit id) is
  RECOMMENDED. A monotonic sequence number issued by the source is acceptable.
  A wall-clock timestamp is NOT sufficient, because it does not distinguish two
  states written in the same tick and does not survive clock adjustment.
- **Source of truth** — the single authority for content. There MUST be exactly
  one per corpus.
- **Derived consumer** — any store or view built from the source and read
  instead of it: search index, projection, cache, replica, mirror.
- **Read** — any operation that returns corpus-derived content to a caller.
- **Currency claim** — an assertion, explicit or implied, that a read reflects
  the present state of the source.

---

## 3. Requirements

### R1 — A read reports its revision

Every read MUST return, in the same structure as its results, the revision of
the source from which those results were derived.

The revision MUST be part of the result payload. Logging it, exposing it on a
separate status endpoint, or recording it in a metrics label does not satisfy
R1: a caller that must make a second, separately-timed call cannot bind the
revision to the answer it already holds.

*Rationale.* A caller cannot evaluate an answer's currency without knowing what
it is current as of. This is the single most-violated requirement in practice.

### R2 — Derived consumers agree with the source

The system MUST provide an operation that compares the revision reported by
every derived consumer against the revision of the source, and reports agreement
or the specific disagreement.

The comparison MUST cover every consumer that can serve a read. A system that
checks its search index but not its replica satisfies R2 only for the index.

*Rationale.* R1 makes a single answer inspectable. R2 makes the system as a
whole inspectable, which is what an operator and an automated agent both need.

### R3 — Disagreement fails closed

When any derived consumer's revision does not match the source, the system MUST
either

- **(a)** refuse the read, or
- **(b)** serve the read carrying an explicit, machine-readable degradation
  label naming the stale consumer and both revisions.

The system MUST NOT serve an unlabelled result while a consumer it read from is
known to disagree.

Rebuilding the stale consumer and then serving satisfies R3, because the read
that is finally served agrees. Serving from the stale consumer while a rebuild
is scheduled does not.

*Rationale.* An answer that is known to be possibly-stale and says so is
recoverable; the caller can wait, retry or escalate. An answer that is
possibly-stale and silent is not.

### R4 — Writes advance one revision at a time

The system SHOULD serialize writes such that every write produces exactly one
new revision, and no read can observe a partially-applied write.

*Rationale.* R1 through R3 are meaningless if a revision identifier can name a
half-written state. Single-writer discipline is the cheapest way to guarantee
this; transactional commit is another.

---

## 4. Conformance levels

| Level | Requirements | What it buys |
|---|---|---|
| **0 — Unverified** | none | Nothing. Staleness is undetectable from outside. |
| **1 — Attributed** | R1 | A caller can tell two answers apart and detect a regression in currency. |
| **2 — Checked** | R1, R2 | An operator or agent can establish, on demand, that the whole system is current. |
| **3 — Fail-closed** | R1, R2, R3 | The system cannot silently serve stale content. |
| **4 — Serialized** | R1, R2, R3, R4 | Revisions name unambiguous states; level 3 guarantees hold under concurrent writes. |

A system claiming a level MUST satisfy every requirement at and below it.

---

## 5. Non-goals

This specification does not address, and conformance implies nothing about:

- **Retrieval quality.** Whether the right documents come back is orthogonal.
- **Tamper evidence.** Proving history was not rewritten is a different and
  complementary property; a level 4 system with no signatures is still
  conformant.
- **Consistency between replicas of the source.** This concerns agreement
  between a source and things derived *from* it, not distributed consensus on
  what the source is.
- **Freshness in wall-clock terms.** Conformance says a system knows and reports
  what it is serving. It does not mandate how quickly derived consumers catch up.

---

## 6. Testing conformance

Conformance is testable from outside the system, without access to its internals.

**R1.** Issue a read. Inspect the returned structure for a revision field.
*Pass:* the identifier is present in the result payload.
*Common failure:* the identifier exists internally but is not returned. Reading
the source is the fastest check — find the type a read returns and enumerate its
fields.

**R2.** Write to the source. Before any rebuild completes, invoke the agreement
operation.
*Pass:* it names the lagging consumer and both revisions.
*Common failure:* no such operation exists, or it reports only liveness.

**R3.** Hold a derived consumer at an old revision. Issue a read that would be
answered from it.
*Pass:* the read is refused, or it returns with an explicit degradation label.
*Common failure:* a normal, unlabelled result.

**R4.** Issue concurrent writes. Read repeatedly throughout.
*Pass:* every observed revision corresponds to a complete write.

A survey applying the R1–R3 tests to nine open-source agent-memory systems on
2026-09-29 found **none at level 1**. Two reached the underlying idea in part:
one keyed derived caches on a corpus counter that never left the process, and
one implemented a signed append-only log that was never bound to its read path.

---

## 7. Applying it to an agent session

The requirements above describe a system. Agents that read from such a system
need a contract of their own, because an agent's failure mode is to carry a
stale fact forward into an action.

A conforming **session start** SHOULD:

1. obtain the source's current revision directly;
2. verify every derived consumer it intends to read agrees (R2);
3. refuse to claim currency if any disagrees, proceeding only against
   explicitly-labelled stale data (R3);
4. state, in its opening report, the revision and what was checked.

A conforming **session end** SHOULD write through the serialized writer (R4),
verify the resulting revision propagated to every consumer, and record the
revision it reached. An unverified write MUST NOT be reported as complete.

The point of both halves is that currency is *established and stated*, never
assumed.

---

## 8. Prior art

- **RFC 6962 / certificate transparency** — append-only Merkle logs with signed
  checkpoints and consistency proofs. Solves tamper evidence; does not bind
  reads to log state.
- **HTTP `ETag` / `If-None-Match`** (RFC 9110) — the closest widely-deployed
  analogue to R1. A representation carries a validator the client can act on.
  Scoped to one resource over one protocol rather than to a corpus with many
  derived consumers.
- **Cache invalidation by version sentinel** — standard practice, internal by
  construction.
- **Read-your-writes and monotonic-read consistency** — describe guarantees
  between replicas of a store. Revision agreement concerns a source and its
  derived views, and adds the requirement that the identifier be *reported*.

---

## 9. Proposal: a declared surface (non-normative)

Requirements R1–R4 describe behaviour. **R1 is observable from outside**, because its
evidence rides along with the answer. **R2 and R3 are not**, and no amount of tooling
fixes that:

- An observer cannot **enumerate** a system's derived consumers. A search index, a
  projection, a replica, a warm cache in a peer process — none of them is discoverable
  from a response.
- An observer cannot **induce** disagreement. Holding a consumer behind requires the
  system's own controls.

This produces an asymmetry worth stating plainly: **a black-box test can falsify
conformance but cannot confirm it.** Finding an unlabelled stale answer proves a
violation. Failing to find one proves nothing.

The section below proposes an **optional** surface that makes R2 and R3 inspectable,
on the model of `robots.txt`, `security.txt` and `/.well-known/`. It is not required for
conformance. It exists so that conformance can be *checked* by someone who did not write
the system.

### 9.1 The revision travels with the answer

Required already by [R1](#r1--a-read-reports-its-revision). Over HTTP, an `ETag` is the
idiomatic carrier and needs no new field. Note that a **weak** validator (`W/"…"`, RFC
9110) asserts semantic equivalence rather than one immutable state, and therefore does
not satisfy R1.

### 9.2 An agreement endpoint

A system MAY expose its consumer set and their revisions at a well-known location:

```
GET /.well-known/revision-agreement
```
```json
{
  "source": "9f1c2e4a…",
  "agree": false,
  "consumers": [
    { "name": "search-index", "revision": "9f1c2e4a…", "agrees": true },
    { "name": "projection",   "revision": "3b77aa10…", "agrees": false }
  ]
}
```

With this, [R2](#r2--derived-consumers-agree-with-the-source) becomes a single request
that anyone can make, and the answer names the laggard. Without it, R2 can only be tested
by someone who already knows what the consumers are.

### 9.3 A degradation label

When a system serves a read despite a known disagreement — branch (b) of
[R3](#r3--disagreement-fails-closed) — the label MUST be machine-readable. In a JSON
envelope:

```json
{ "results": [], "revision": "3b77aa10…",
  "stale": { "consumer": "search-index", "served": "3b77aa10…", "source": "9f1c2e4a…" } }
```

or as a response header:

```
Revision-Agreement: stale; consumer=search-index; served=3b77aa10; source=9f1c2e4a
```

A prose warning in a message string does not qualify. The caller acting on the answer is
usually a program.

### 9.4 What this still does not solve

Confirming R3 requires inducing disagreement, which remains system-specific. Standardising
a fault-injection hook is deliberately **not** proposed here: an endpoint whose purpose is
to make a system serve stale data is an attack surface, and the cure would be worse than
the disease.

One generic technique falsifies R3 without any hook. Write, note the resulting revision,
then read repeatedly. **Any read carrying a pre-write revision and no degradation label is
a violation**, caught with no privileged access at all. The race window may be narrower
than the sampling rate, so this finds real violations and certifies nothing — which is the
asymmetry above, restated.

### 9.5 Status

Non-normative and unimplemented. It is recorded here because "how would anyone check
this?" is a fair question to ask of any specification, and because the answer shapes the
requirements. Counter-proposals are more useful than agreement.

---

## 10. Licence

This specification is released under CC BY 4.0. Implementations are unencumbered.

