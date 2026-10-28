# Configured-host diagnostic record — profile 1

The first run of [the frozen profile](configured-host-diagnostics.md) completed on
2026-10-27 at source `25bd4dc11cc50d4f2b5460a025012bc5110450f3`, tree
`8a5874eb598602aabbea7126eacbcef5b00ad484`. The profile, fixtures and measurement
code were committed before the run. No measurement was repeated and neither
provisional target was changed after seeing results.

Question: does this bounded native reference path fit the provisional 250 ms p95
and 64 MiB child process-lifetime RSS evaluation targets on the disclosed machine?
All nine cells were within those targets. This is not adoption of product
guarantees, a browser result, human evidence or completion of every MVP gate.

## Observations

| Cell | p95 ms | Maximum ms | Peak child RSS MiB |
| --- | ---: | ---: | ---: |
| Visit | 2.879 | 3.843 | 8.375 |
| Equal-time due batch | 1.729 | 1.874 | 8.375 |
| Save pending | 3.775 | 4.019 | 8.375 |
| Fresh-process load pending | 1.824 | 2.322 | 8.375 |
| Live-host `restore_into` | 12.682 | 19.946 | 8.375 |
| Catch-up to hour 1000 | 9.333 | 12.451 | 8.625 |
| Typed capacity refusal | 2.120 | 2.701 | 8.375 |
| Historical near-text load | 6.445 | 13.581 | 8.375 |
| Historical near-text save | 10.630 | 10.866 | 8.375 |

Each cell has 50 measured samples and five retained warmups. Latency summaries
exclude warmups; the reported RSS peak includes them. Across all 495 processes,
440 exited successfully and 55 returned the expected typed `delivery_sequence`
capacity refusal. There were no execution or functional failures. Raw nanosecond
values, maxima and all captured outputs were retained, including slower samples.

The fresh-load cell reads a small pending fixture into a new CLI process. The live
replacement cell retains an existing pressure host while validating the historical
near-text candidate. Its full-process numbers also include test harness and
canonical verification overhead. These are distinct workloads, not a controlled
comparison of two implementations. The separately recorded inner replacement
duration remains a diagnostic with no independent target.

The selected Linux x86_64 environment reported five eligible logical CPUs, a
four-CPU quota, 16 GiB cgroup memory and overlayfs. Shared scheduling, cache state
and underlying storage were not isolated. Rust 1.99.1 release artifacts and both
binary hashes, all 14 fixture hashes, source identity and raw environment fields
are retained in the report. Source and fixture identities were checked again
after measurement.

## Memory interpretation

These are **child process-lifetime RSS peaks**, not exact post-exec Rust or Core
peaks. Linux retains the previous address space's high-water mark across exec,
and CPython's subprocess path uses fork/vfork before exec. The readings therefore
include possible pre-exec Python-wrapper memory. The common 8.375 MiB floor is
consistent with that mechanism; its exact contribution cannot be recovered from
this run. No guessed baseline is subtracted. The provisional target classification
holds for this broader process boundary. See the
[Linux exec implementation](https://raw.githubusercontent.com/torvalds/linux/v6.18/fs/exec.c)
and [CPython subprocess implementation](https://raw.githubusercontent.com/python/cpython/v3.12.14/Modules/_posixsubprocess.c).

## Evidence and remaining uncertainty

The original files remain outside Git, unchanged:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `report.json` | 178362 | `3cea96febe6193cd503fc38da7c9c44f8cdb3a40d41a13c50d85d266736b701e` |
| `raw-samples.jsonl` | 98116 | `17969d4f7f56d375b7f69ba7bdeaad53a1c34191ea9505fee8c4da0a84ce00b3` |

Independent review recomputed every statistic and classification, matched all 495
raw rows to captured wrapper results, checked source/binary/fixture hashes and
accounted for all 2,145 raw files. Review also checked actual equal-time receipts,
unchanged refusal reports, canonical saves, catch-up sequences and live replacement
results. No workload, target or recorded measurement was changed after the run;
review identified the RSS interpretation caveat above.

This resolves a missing diagnostic record for the configured native host. It does
not measure isolated Core heap, cold storage, browser input-to-paint, file-dialog
or keyboard/accessibility behavior, other hosts, larger worlds, independent
authoring or player value. No Site, upload retry, deployment or larger-game work
was part of this run.
