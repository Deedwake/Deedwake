#!/usr/bin/env python3
"""Opt-in Linux diagnostics of actual Rust processes; contains no simulation."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import resource
import subprocess
import sys
import tempfile
import unittest

PROFILE = "configured_convoy_diagnostic_v1"
WARMUPS, SAMPLES = 5, 50
TARGET_NS, TARGET_RSS = 250_000_000, 64 * 1024 * 1024
WORKER = "convoy_session::acceptance::qualification_worker"
REPOSITORY = Path(__file__).resolve().parents[1]

# One fresh interpreter per Rust child makes RUSAGE_CHILDREN a per-process peak.
# No Python model or sampled /proc polling stands in for actual Rust behavior.
PROCESS_WRAPPER = r'''
import json, os, resource, subprocess, sys, time
r = json.load(sys.stdin)
assert resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss == 0
env = os.environ.copy(); env.update(r["environment"])
with open(r["stdout"], "xb") as out, open(r["stderr"], "xb") as err:
    start = time.perf_counter_ns()
    child = subprocess.run(r["command"], stdout=out, stderr=err, env=env)
    elapsed = time.perf_counter_ns() - start
print(json.dumps({"elapsed_ns": elapsed, "rss_bytes":
    resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss * 1024,
    "exit": child.returncode}))
'''


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def checked(command, **kwargs):
    return subprocess.run(command, check=True, text=True, **kwargs)


def output(command):
    return checked(command, cwd=REPOSITORY, capture_output=True).stdout.strip()


def build_overrides(environment):
    exact = {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTDOC", "RUSTDOCFLAGS",
             "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTC_BOOTSTRAP"}
    return [key for key, value in environment.items() if value and (
        key in exact or key.startswith(("CARGO_PROFILE_", "CARGO_BUILD_"))
        or key.startswith("CARGO_TARGET_") and key != "CARGO_TARGET_DIR")]


def cargo_configs(repository, environment):
    home = Path(environment.get("CARGO_HOME", str(Path.home() / ".cargo")))
    directories = [home] + [p / ".cargo" for p in (repository, *repository.parents)]
    return sorted({str(p / name) for p in directories for name in ("config", "config.toml")
                   if (p / name).exists()})


def summary(samples, warmups):
    if len(samples) != SAMPLES or len(warmups) != WARMUPS:
        raise ValueError("incomplete cell cannot receive a target verdict")
    times = sorted(s["elapsed_ns"] for s in samples)
    rss = max(s["rss_bytes"] for s in samples + warmups)
    p95 = times[math.ceil(.95 * len(times)) - 1]
    return {"measured_sample_count": len(samples), "warmup_count": len(warmups),
            "median_ns": times[math.ceil(.50 * len(times)) - 1],
            "p95_ns": p95, "max_ns": times[-1], "peak_rss_bytes": rss,
            "within_provisional_targets": p95 <= TARGET_NS and rss <= TARGET_RSS}


def append_raw(stream, event):
    stream.write(json.dumps(event, sort_keys=True) + "\n")
    stream.flush()
    os.fsync(stream.fileno())


def worker_command(binary):
    return [str(binary), WORKER, "--exact", "--ignored", "--nocapture", "--test-threads=1"]


def compile_binary(arguments, log, *, test):
    result = subprocess.run(arguments + ["--message-format=json"], cwd=REPOSITORY,
                            capture_output=True, text=True)
    log.write_text(result.stdout + "\n" + result.stderr)
    result.check_returncode()
    executables = []
    for line in result.stdout.splitlines():
        item = json.loads(line)
        if (item.get("reason") == "compiler-artifact"
                and item.get("target", {}).get("name") == "convoy-tool"
                and item.get("profile", {}).get("test") == test
                and item.get("executable")):
            executables.append(Path(item["executable"]).resolve())
    if len(executables) != 1:
        raise RuntimeError("cannot identify one freshly built release executable")
    executable = executables[0]
    if "release" not in executable.parts:
        raise RuntimeError("measurement requires release artifacts")
    return executable


def cli_cells(binary, fixtures):
    def command(name, *args):
        return [str(binary), "--load", str(fixtures / name), *args, "--json"]
    return [
        ("visit", command("held.json", "--action", "visit"), 0, 3),
        ("equal_due_batch", command("equal.json", "--action", "wait"), 0, 1),
        ("save_pending", command("pending.json"), 0, 3),
        ("fresh_load_pending", command("pending.json"), 0, 3),
        ("live_restore_into", [], 0, None),
        ("catchup_to_1000", command("pending.json", *(["--action", "wait"] * 997),
                                    "--action", "visit"), 0, 1000),
        ("capacity_refusal", command("pressure.json", "--action", "ask"), 1, 0),
        ("old_text_load", command("oldneartext.json"), 0, 4),
        ("old_text_save", command("oldneartext.json"), 0, 4),
    ]


def verify_result(name, sample, stdout, worker_result, destination, fixtures, expected_exit,
                  expected_time, pressure_reference):
    if sample["exit"] != expected_exit:
        raise ValueError(f"{name}: unexpected process exit {sample['exit']}")
    if sample["rss_bytes"] <= 0 or sample["elapsed_ns"] <= 0:
        raise ValueError("invalid measurement")
    if name == "live_restore_into":
        value = json.loads(worker_result.read_text())
        result = value.get("result", value)
        if not (value.get("status") == "passed" and result.get("canonical_byte_equal")
                and result.get("player_output_generated") and result.get("inner_ns", 0) > 0):
            raise ValueError("live restore did not prove the real replacement boundary")
        sample["restore_inner_ns_diagnostic"] = result["inner_ns"]
        return
    value = json.loads(stdout.read_text())
    if value.get("time") != expected_time or value.get("ok") != (expected_exit == 0):
        raise ValueError(f"{name}: unexpected host result/time")
    if name == "equal_due_batch":
        receipts = value.get("receipts", [])
        if ([r.get("route") for r in receipts] != ["report", "routine", "intermediary"]
                or not all(r.get("state") == "delivered" and r.get("at") == 1
                           and r.get("due") == 1 for r in receipts)):
            raise ValueError("equal-time batch did not actually arrive")
    if name == "capacity_refusal":
        if not value["steps"][-1].get("capacity_resource"):
            raise ValueError("expected typed capacity refusal")
        projection = {k: v for k, v in value.items() if k not in ("ok", "steps")}
        if projection != pressure_reference:
            raise ValueError("capacity refusal changed visible native report")
    if name in ("save_pending", "old_text_save"):
        source = fixtures / ("pending.json" if name == "save_pending" else "oldneartext.json")
        if destination.read_bytes() != source.read_bytes():
            raise ValueError("saving loaded fixture changed canonical native bytes")


def qualify(directory):
    if sys.platform != "linux":
        raise RuntimeError("RSS units and native atomic-file policy require Linux")
    if not directory.is_absolute() or directory.exists():
        raise ValueError("output must be a NEW absolute directory")
    parent = directory.parent.resolve(strict=True)
    if parent == REPOSITORY or REPOSITORY in parent.parents:
        raise ValueError("diagnostic output must be outside the checkout")
    if output(["git", "status", "--porcelain", "--untracked-files=all"]):
        raise RuntimeError("freeze profile in a clean commit before measurement")
    if build_overrides(os.environ):
        raise RuntimeError("custom build overrides are outside the frozen profile")
    configurations = cargo_configs(REPOSITORY, os.environ)
    if configurations:
        raise RuntimeError("Cargo configuration requires profile review before measurement: "
                           + ", ".join(configurations))
    rustc = output(["rustc", "-Vv"])
    if "release: 1.99.1" not in rustc.splitlines():
        raise RuntimeError("pinned Rust 1.99.1 is required")
    head = output(["git", "rev-parse", "HEAD"])
    directory.mkdir()
    logs, fixtures, raw = directory / "logs", directory / "fixtures", directory / "raw"
    for p in (logs, fixtures, raw):
        p.mkdir()
    report = {"schema_version": 1, "profile": PROFILE, "status": "preparing",
              "target_kind": "provisional_diagnostic_evaluation_not_product_guarantee",
              "source": {"commit": head, "tree": output(["git", "rev-parse", "HEAD^{tree}"])},
              "targets": {"p95_ns": TARGET_NS, "peak_rss_bytes": TARGET_RSS,
                          "warmups": WARMUPS, "measured_samples": SAMPLES,
                          "rss_scope": "maximum over warmups AND measured fresh Rust processes"},
              "environment": {"platform": platform.platform(), "python": sys.version,
                              "rustc": rustc, "cargo": output(["cargo", "-V"]),
                              "filesystem": output(["stat", "-f", "-c", "%T", str(parent)]),
                              "cpu_affinity_count": len(os.sched_getaffinity(0)),
                              "external_cargo_configs": configurations,
                              "build_environment_overrides": []},
              "limitations": ["No cold-disk, isolated-CPU or universal latency claim",
                              "Whole-process RSS is not Core heap attribution",
                              "No browser, human, larger-game or release acceptance",
                              "CLI read has no old live host; live restore is a separate cell"],
              "cells": []}
    for name, path in {"cpuinfo": "/proc/cpuinfo", "meminfo": "/proc/meminfo",
                       "cpu_quota": "/sys/fs/cgroup/cpu.max",
                       "memory_limit": "/sys/fs/cgroup/memory.max"}.items():
        text = Path(path).read_text()
        report["environment"][name] = (next(line for line in text.splitlines()
                                             if line.startswith("model name"))
                                          if name == "cpuinfo" else text)
    def persist():
        temporary = directory / "report.json.next"
        temporary.write_text(json.dumps(report, indent=2) + "\n")
        os.replace(temporary, directory / "report.json")
    persist()
    with (directory / "raw-samples.jsonl").open("x") as journal:
        try:
            binary = compile_binary(["cargo", "build", "-p", "deedwake-repair-tool", "--bin",
                                     "convoy-tool", "--release", "--locked", "--offline"],
                                    logs / "build-cli.log", test=False)
            worker = compile_binary(["cargo", "test", "-p", "deedwake-repair-tool", "--bin",
                                     "convoy-tool", "--release", "--no-run", "--locked", "--offline"],
                                    logs / "build-worker.log", test=True)
            report["binaries"] = {"cli": {"path": str(binary), "sha256": sha(binary)},
                                  "worker": {"path": str(worker), "sha256": sha(worker)}}
            env = os.environ.copy()
            env.update(DEEDWAKE_CONVOY_QUAL_MODE="prepare", DEEDWAKE_CONVOY_QUAL_DIR=str(fixtures),
                       DEEDWAKE_CONVOY_QUAL_RESULT=str(directory / "prepare.json"))
            with (logs / "prepare.stdout").open("x") as out, (logs / "prepare.stderr").open("x") as err:
                checked(worker_command(worker), env=env, stdout=out, stderr=err)
            report["fixtures"] = {p.name: {"sha256": sha(p), "bytes": p.stat().st_size}
                                  for p in sorted(fixtures.iterdir()) if p.is_file()}
            report["preparation"] = json.loads((directory / "prepare.json").read_text())
            if report["preparation"].get("status") != "passed":
                raise RuntimeError("fixture correctness failed")
            reference_process = subprocess.run([str(binary), "--load", str(fixtures / "pressure.json"),
                                                "--json"], capture_output=True, text=True)
            (logs / "pressure-reference.stdout").write_text(reference_process.stdout)
            (logs / "pressure-reference.stderr").write_text(reference_process.stderr)
            reference_process.check_returncode()
            reference = json.loads(reference_process.stdout)
            pressure_reference = {k: v for k, v in reference.items() if k not in ("ok", "steps")}
            report["status"] = "measuring"
            persist()
            for name, command, expected_exit, expected_time in cli_cells(binary, fixtures):
                cell = {"name": name, "status": "running", "warmups": [], "samples": []}
                report["cells"].append(cell)
                for index in range(WARMUPS + SAMPLES):
                    prefix = raw / f"{name}-{index:02d}"
                    stdout, stderr = prefix.with_suffix(".stdout"), prefix.with_suffix(".stderr")
                    destination, result_path = prefix.with_suffix(".save"), prefix.with_suffix(".worker.json")
                    arguments = command.copy()
                    environment = {}
                    if name == "live_restore_into":
                        arguments = worker_command(worker)
                        environment = {"DEEDWAKE_CONVOY_QUAL_MODE": "restore_into",
                                       "DEEDWAKE_CONVOY_QUAL_DIR": str(fixtures),
                                       "DEEDWAKE_CONVOY_QUAL_RESULT": str(result_path)}
                    elif name in ("save_pending", "old_text_save"):
                        arguments += ["--save", str(destination)]
                    request = {"command": arguments, "environment": environment,
                               "stdout": str(stdout), "stderr": str(stderr)}
                    response = subprocess.run([sys.executable, "-I", "-c", PROCESS_WRAPPER],
                                              input=json.dumps(request), capture_output=True, text=True)
                    prefix.with_suffix(".wrapper.stdout").write_text(response.stdout)
                    prefix.with_suffix(".wrapper.stderr").write_text(response.stderr)
                    if response.returncode:
                        raise RuntimeError(f"measurement wrapper failed; retained output for {prefix.name}")
                    sample = json.loads(response.stdout)
                    sample.update(index=index, phase="warmup" if index < WARMUPS else "sample",
                                  name=name, stdout=str(stdout.relative_to(directory)),
                                  stderr=str(stderr.relative_to(directory)))
                    append_raw(journal, sample)
                    cell["warmups" if index < WARMUPS else "samples"].append(sample)
                    persist()
                    verify_result(name, sample, stdout, result_path, destination, fixtures,
                                  expected_exit, expected_time, pressure_reference)
                cell.update(summary(cell["samples"], cell["warmups"]), status="complete")
                persist()
            if (output(["git", "rev-parse", "HEAD"]) != head
                    or output(["git", "status", "--porcelain", "--untracked-files=all"])):
                raise RuntimeError("source changed during diagnostic run")
            if sha(binary) != report["binaries"]["cli"]["sha256"] or sha(worker) != report["binaries"]["worker"]["sha256"]:
                raise RuntimeError("release executable changed during diagnostic run")
            if any(sha(fixtures / name) != value["sha256"] for name, value in report["fixtures"].items()):
                raise RuntimeError("frozen fixture changed during diagnostic run")
            report["status"] = ("within_provisional_targets" if all(
                c["within_provisional_targets"] for c in report["cells"])
                else "outside_provisional_targets")
            persist()
        except BaseException as error:
            report["status"] = "execution_error"
            report["error"] = str(error)
            if isinstance(error, subprocess.CalledProcessError):
                report["failed_process"] = {"command": error.cmd, "exit": error.returncode,
                                            "stdout": error.stdout, "stderr": error.stderr}
            if report["cells"] and report["cells"][-1]["status"] == "running":
                report["cells"][-1]["status"] = "execution_error"
            append_raw(journal, {"event": "execution_error", "error": str(error)})
            persist()
            raise
    return 0 if report["status"] == "within_provisional_targets" else 1


class DiagnosticTests(unittest.TestCase):
    def test_build_override_sources_are_not_silently_accepted(self):
        for key in ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTFLAGS",
                    "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS", "RUSTC_BOOTSTRAP"):
            self.assertEqual(build_overrides({key: "value"}), [key])
        self.assertEqual(build_overrides({"CARGO_TARGET_DIR": "/tmp/target", "RUSTUP_HOME": "/tmp/rustup"}), [])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / ".cargo").mkdir()
            (root / ".cargo/config.toml").write_text("[build]\nrustflags=[]\n")
            self.assertIn(str(root / ".cargo/config.toml"), cargo_configs(root, {"CARGO_HOME": str(root)}))

    def test_nearest_rank_inclusive_targets_and_warmup_peak(self):
        samples = [{"elapsed_ns": n, "rss_bytes": TARGET_RSS} for n in range(1, 51)]
        warmups = [{"elapsed_ns": TARGET_NS * 2, "rss_bytes": 1}] * WARMUPS
        result = summary(samples, warmups)
        self.assertEqual((result["median_ns"], result["p95_ns"], result["max_ns"]), (25, 48, 50))
        self.assertTrue(result["within_provisional_targets"])
        warmups[0] = {"elapsed_ns": 1, "rss_bytes": TARGET_RSS + 1}
        self.assertFalse(summary(samples, warmups)["within_provisional_targets"])
        for sample in samples:
            sample["elapsed_ns"] = TARGET_NS
        self.assertTrue(summary(samples, warmups[1:] + [warmups[1]])["within_provisional_targets"])
        samples[0]["elapsed_ns"] = TARGET_NS + 1
        self.assertEqual(summary(samples, warmups)["max_ns"], TARGET_NS + 1)
        for sample in samples[:4]:
            sample["elapsed_ns"] = TARGET_NS + 1
        self.assertFalse(summary(samples, [{"elapsed_ns": 1, "rss_bytes": 1}] * WARMUPS)
                         ["within_provisional_targets"])

    def test_incomplete_cells_never_receive_pass_verdict(self):
        with self.assertRaises(ValueError):
            summary([], [])

    def test_expected_capacity_exit_is_not_a_launch_failure(self):
        with self.assertRaisesRegex(ValueError, "unexpected process exit"):
            verify_result("capacity_refusal", {"exit": 2}, None, None, None, None, 1, 0, {})

    def test_empty_equal_time_receipts_are_not_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "output"
            path.write_text(json.dumps({"time": 1, "ok": True, "receipts": []}))
            with self.assertRaisesRegex(ValueError, "did not actually arrive"):
                verify_result("equal_due_batch", {"exit": 0, "rss_bytes": 1, "elapsed_ns": 1},
                              path, None, None, None, 0, 1, {})

    def test_raw_sample_survives_later_validation_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "raw.jsonl"
            event = {"elapsed_ns": TARGET_NS + 1, "rss_bytes": TARGET_RSS + 1, "exit": 2}
            with path.open("x") as stream:
                append_raw(stream, event)
                with self.assertRaises(ValueError):
                    verify_result("capacity_refusal", event, None, None, None, None, 1, 0, {})
            self.assertEqual(json.loads(path.read_text()), event)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.self_test:
        unittest.main(argv=[sys.argv[0]], exit=True)
    if args.output is None:
        parser.error("--output NEW_ABSOLUTE_DIRECTORY is required")
    sys.exit(qualify(args.output))
