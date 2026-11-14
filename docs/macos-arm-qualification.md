# macOS ARM qualification boundary

The additional native target is macOS ARM (`aarch64-apple-darwin`) on local APFS.
It is not a claim for other architectures, network filesystems, concurrent writers,
or universal power-loss durability. Linux keeps its existing native path and
frozen diagnostic profile. No dependency, snapshot format, Core semantics or
browser checkpoint contract changes for this target. The writer's OS/architecture
gate does not inspect filesystem type; callers must choose the qualified local
APFS destination. Successful calls on another filesystem do not qualify it.

## Native file replacement

Both native hosts use the same bounded writer: validate/serialize before writing,
create a new sibling temporary file with requested mode `0600`, write and
`File::sync_all`, atomically rename over the destination, then synchronize the
parent directory. Errors before rename preserve the existing destination and
remove the temporary. Failure after rename reports `DurabilityUncertain`: the
replacement is already committed and the old contents must not be promised.

[Pinned Rust 1.99.1's Apple implementation](https://github.com/rust-lang/rust/blob/1.99.1/library/std/src/sys/fs/unix.rs#L1180-L1190)
of `File::sync_all` uses `F_FULLFSYNC`.
The port does not substitute an unchecked flush, ignore directory-sync errors,
or add FFI. Tests exercise real file/directory synchronization, sibling placement,
requested mode, stage order, each injected precommit failure and postcommit
uncertainty. Cross-process session tests establish restoration behavior separately
from filesystem tests. Neither proves survival of arbitrary hardware power loss.

Requested `0600` mode bits do not establish denial of access by another local
user when filesystem ownership enforcement is disabled. Verify the destination's
ownership and access policy separately before storing private snapshots. This
workflow does not change volume or security settings.

[APFS requires valid UTF-8 filename bytes](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/APFS_Guide/FAQ/FAQ.html). A non-UTF-8 CLI path remains an OS-path
argument, but APFS rejects file creation and the CLI reports a structured read
error (exit 2), not a syntax/usage error (exit 64). Linux byte-filename success
coverage is unchanged; valid Unicode filenames are tested separately.

## Separate Darwin diagnostic profile

`scripts/qualify-convoy.py` identifies Darwin results as
`configured_convoy_diagnostic_darwin_arm64_v1`, not the frozen Linux
`configured_convoy_diagnostic_v1`. It retains the same nine real native operation
cells, 5 latency warmups, 50 measured samples and provisional numeric targets.
Equal targets do not establish equivalent performance across operating systems.

[Darwin `RUSAGE_CHILDREN.ru_maxrss`](https://github.com/apple/darwin-xnu/blob/main/bsd/man/man2/getrusage.2)
is already bytes; Linux values are KiB and
are multiplied by 1,024. CPU model, physical memory and logical CPU count are
read from `sysctl`; filesystem type and ownership status come from `diskutil`
at the actual mount root. Non-APFS Darwin measurements are rejected. Unavailable
Linux affinity/cgroup controls remain `null` with an explicit explanation, never
fabricated zeroes or unlimited-resource claims. Missing required evidence fails
before building/sampling and retains an execution-error report.

Run from a clean committed checkout with pinned tools, existing locked caches,
and a new absolute output directory outside the checkout:

```sh
python3 -B scripts/qualify-convoy.py --self-test
python3 -B scripts/qualify-convoy.py --output "$OUTPUT_DIRECTORY"
```

Use a separate Cargo target directory for each checkout during qualification.
The npm Wasm build expects its outputs in checkout-local `target/`; retain that
layout or use a distinct per-checkout target link. Do not share build artifacts
between different source checkouts. Keep diagnostic output outside tracked source.
The older ignored Repair acceptance benchmark remains explicitly Linux-only;
this port does not reinterpret its frozen provenance or performance results.

Native correctness, actual Wasm/TypeScript parity, interactive browser behavior,
and human acceptance are separate checks. A browser click cannot establish
native lifecycle/restoration or filesystem durability.
