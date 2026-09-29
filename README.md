# Revision Agreement

**A system that answers from a copy should be able to tell you which copy.**

Most systems that answer questions from a corpus keep the real content in one
place and build a search index on top of it. The index is a copy, and copies
lag. When they do, a healthy-looking system returns a well-formed, confident,
out-of-date answer, and nothing in that answer says so.

This repository specifies the property that makes that failure visible, and
how to test for it.

**[Read the specification →](SPEC.md)**  ·  **[Run the checker →](REVCHECK.md)**

```console
$ curl -sD - https://example.test/search?q=x | revcheck
level 0 (Unverified)
near-misses:
- header:Last-Modified: rejected by name (wall-clock field)
- $.results[0].updated_at: rejected by name (wall-clock field)
- $.results[0].hash: per-item scope
```

Pass `curl -D -` and it reads the response headers too, because the most widely deployed
revision identifier in existence is an `ETag`. A **weak** validator (`W/"…"`) is reported
as a near-miss: RFC 9110 says it asserts semantic equivalence, not one immutable state.

`revcheck` reads a response body and decides whether it satisfies **R1**. It is
deliberately narrow: it tests R1 and revision advance, and **not** R2 or R3, which need
the system's own write and consumer operations and cannot be judged from a payload.

Two rules do most of the work, and both come straight from the specification:

- **Envelope, not item.** A revision on each result identifies that result, not the state
  of the store, so it cannot satisfy R1. A per-item content hash is reported as a
  near-miss rather than a pass.
- **Timestamps are not revisions.** [§2](SPEC.md#2-terminology) rules out wall-clock time,
  so an ISO-8601 value is rejected even under a name like `version`.

**A pass is weaker than a failure.** `revcheck` sees shape, not meaning: an API version and
a corpus revision are indistinguishable in a single payload. A failure is therefore the
stronger verdict — nothing revision-shaped is present at all — while a pass should be
confirmed with `revcheck advance` around a write, which shows whether the value actually
moves with the data.

**Can this ever test R2 and R3?** Not from outside, and [§9](SPEC.md#9-proposal-a-declared-surface-non-normative)
explains why: an observer can neither enumerate a system's derived consumers nor induce
disagreement between them. A black-box test can therefore falsify conformance but never
confirm it. That section proposes an optional declared surface that would make R2 and R3
checkable by someone who did not write the system.

When nothing qualifies it prints the near-misses and why each was rejected, because
"no" is only useful if you can see what it nearly matched.

---

## The three questions

Ask them of any system that serves content through a derived copy — a search
index, a projection, a cache, a replica.

1. **Does a result carry its revision?** When an answer comes back, is the
   revision it was derived from in the payload, next to the content?
2. **Can the copy be checked against the original?** Is there an operation that
   compares every derived consumer's revision against the source and names the
   laggard?
3. **Does disagreement stop it?** When they disagree, does the system refuse or
   label the answer, or does it serve from the copy it knows is behind?

A system that does all three can be checked by its callers. A system that does
none has to be taken on trust.

The specification turns these into [conformance levels 0–4](SPEC.md#4-conformance-levels).

---

## The ten-minute test

You do not need to run anything.

1. Find the function that answers a search or read call.
2. Follow it to the structure it returns and read the field names. You are
   looking for a commit hash, revision id, snapshot number, sequence number or
   ETag. If every field is content or scoring, the caller cannot check anything.
3. Search the whole project for that same vocabulary. If the only hits are in
   tests, migrations, documentation tooling or a benchmark harness, it is not in
   the serving path.

What you are hoping to find is the revision sitting in the same structure as the
answer.

[Full test procedure, including R2 and R3 →](SPEC.md#6-testing-conformance)

---

## Why this is separate from the things it resembles

Two mature traditions solve adjacent problems and stop before this one.

**Cache invalidation** keeps derived state correct inside a process by keying it
on a version sentinel. The sentinel is an implementation detail, motivated by
speed, usually process-local, and not reported to the caller.

**Transparency logs** make write history verifiable after the fact through
append-only structures, signed checkpoints and consistency proofs. They prove
history was not rewritten. They do not bind a read to the state that produced it.

Revision agreement sits between them: carry the version across the boundary to
the reader, at the moment the reader is about to act on it.

See [§8 Prior art](SPEC.md#8-prior-art) for how this relates to RFC 6962, HTTP
`ETag`, and read-your-writes consistency.

---

## Status

Draft 0.1.0. The specification is stable enough to test against and is expected
to change in response to implementations and counterexamples.

**Counterexamples are the most useful contribution.** If you know a system that
returns its revision with its results, open an issue — that is a system this
specification should cite rather than describe the absence of.

---

## Licence

The specification is released under [CC BY 4.0](LICENSE). Implementations are
unencumbered.
