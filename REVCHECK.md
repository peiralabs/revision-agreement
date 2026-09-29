# revcheck

`revcheck` is the small reference implementation of the R1 test in
[`SPEC.md`](SPEC.md): it checks whether a JSON read response reports the
revision from which it was served. It can also compare two independently
attributed payloads and assert that the revision at a common JSON path changed.

```text
revcheck [FILE]
revcheck --json [FILE]
revcheck advance A B
```

Omit `FILE`, or use `-`, to read standard input. Exit status 0 means the tested
requirement is satisfied, 1 means it is not satisfied, and 2 means the command
or JSON input could not be parsed.

## Scope

This tool tests R1 and revision advance only. It cannot generically test R2
(derived consumers agree with the source) or R3 (disagreement fails closed),
because those tests require the system's own write, agreement, and consumer
operations. A successful result establishes level 1 (Attributed) only; it does
not measure or imply conformance levels 2 through 4.

