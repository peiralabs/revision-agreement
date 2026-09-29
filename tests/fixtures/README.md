# Representative response fixtures

These JSON documents are representative response shapes derived from public
type definitions. They are synthetic examples, not captured traffic, and make
no claim about any named tool or deployment.

| Fixture | Expected result |
|---|---|
| `memory-results.json` | level 0; per-item `hash` scope near-miss |
| `timestamped-results.json` | level 0; `updated_at` rejected by name |
| `dataset-result.json` | level 0; no near-miss |
| `conforming-hex40.json` | level 1; top-level `hex40` revision |
| `conforming-hex40-changed.json` | level 1; different top-level revision |
| `conforming-meta-integer.json` | level 1; nested integer revision |
| `iso-version.json` | level 0; ISO-8601 value near-miss |
| `malformed.json` | parse error |

## Scope of the fixture suite

The suite tests R1 and revision advance only. R2 and R3 require a system's own
write, agreement, and derived-consumer operations, so response fixtures cannot
test them. Passing these cases establishes no claim about conformance levels 2
through 4.

