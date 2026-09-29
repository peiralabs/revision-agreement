TITLE: recall: return the corpus version with results so a caller can tell which state answered

LABELS: enhancement (applied automatically by the feature-request template)

---

### Problem to solve

Remnic already tracks a corpus version, but a caller never sees it. When `recall`
returns results, nothing in the response says which state of the corpus produced them.

That leaves a caller unable to separate two situations: results computed after its own
most recent write, and results computed before it. In the window between a write and the
index catching up, a search answers from the earlier state, and the response looks
identical either way.

This is the same shape as #3082, one axis over. There the problem was that
`count: 0, results: []` could mean "nothing relevant exists" or "retrieval failed, so we
don't know", and a caller handed the first would proceed confidently having "checked".
Same here: an agent instructed to consult memory before acting does consult it, gets a
well-formed answer built from a pre-write state, and proceeds.

The design is already right. The README is explicit that the files are the source of
truth and the index is downstream and rebuildable, never authoritative. The only gap is
that a reader cannot see which downstream state answered it.

### Proposed solution

Return `getMemoryCorpusVersion()` on the recall/search response envelope.

The value already exists and is already durable:

- computed at `packages/remnic-core/src/storage.ts:2421`
- backed by a sentinel file rather than process memory — `readSharedVersion` returns
  `statSync(filePath).size`, with the in-memory map only as a fallback — so peer
  processes already agree on it
- already used to key derived caches at `packages/remnic-core/src/verified-recall.ts:116`,
  with the comment explaining that keying on the wrong sentinel would let a peer process
  serve a stale episode map omitting newly created memories

So this is additive: one field on the response, no behaviour change, backwards compatible.

With it, a caller can keep the version it saw when it wrote, compare it to the version on
a later result, and tell whether it is looking at an answer that predates its own write,
without a second call.

### Alternatives considered

**A separate status call.** Doesn't close it. The caller cannot bind a separately-timed
status response to the result it already holds; the two calls race, and that race is
precisely the window in question.

**Per-memory timestamps.** `updated_at` describes one memory, not the state that was
searched, so it cannot indicate that a memory is missing from the answer.

**Leave it as is.** Reasonable when the caller is a human who will notice something is
off. Harder for agents, which is most of the plugin surface.

### Impact

Anyone building an agent on Remnic, particularly through the plugins where a model
consumes results directly and has no way to sense that an answer is behind.

Measurable: a caller that writes and then immediately recalls can tell from the response
alone that the result predates its write. Today it cannot.

---

Happy to send the patch if you want it. I held off because I wasn't sure whether you'd
prefer the field on the top-level envelope or alongside each result set, and CONTRIBUTING
asks for an issue first on anything touching a surface contract.
