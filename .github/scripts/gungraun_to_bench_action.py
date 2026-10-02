#!/usr/bin/env python3
# Copyright (c) 2019-2026 Provable Inc.
# This file is part of the snarkVM library.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at:
#
# http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""Turn Gungraun summary schema v7 into github-action-benchmark's custom format.

Each input line is one ``BenchmarkSummary``. Callgrind instructions are
``profiles[].data.total.metrics.Ir.values.new``.
"""

import json
import sys


def instruction_count(summary: dict) -> int:
    """Return the new Callgrind instruction count from one Gungraun summary."""
    module_path = summary.get("module_path", "<unknown>")
    for profile in summary.get("profiles") or []:
        if profile.get("tool") != "Callgrind":
            continue
        try:
            value = profile["data"]["total"]["metrics"]["Ir"]["values"]["new"]
        except (KeyError, TypeError) as error:
            raise SystemExit(f"{module_path}: Callgrind summary has no Ir total ({error})") from error
        if isinstance(value, bool) or not isinstance(value, int):
            raise SystemExit(f"{module_path}: Callgrind Ir is not an integer")
        return value
    raise SystemExit(f"{module_path}: summary has no Callgrind profile")


def benchmark_name(summary: dict) -> str:
    module_path = summary["module_path"]
    bench_id = summary.get("id")
    if bench_id:
        return f"{module_path}/{bench_id}"
    return module_path


def convert(lines: list[str]) -> list[dict]:
    results = []
    for line_number, line in enumerate(lines, start=1):
        line = line.strip()
        if not line:
            continue
        try:
            summary = json.loads(line)
        except json.JSONDecodeError as error:
            raise SystemExit(f"line {line_number} is not JSON: {error}") from error
        results.append(
            {
                "name": benchmark_name(summary),
                "unit": "instructions",
                "value": instruction_count(summary),
            }
        )
    if not results:
        raise SystemExit("no Gungraun summaries to record")
    return results


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(f"usage: {sys.argv[0]} INPUT.jsonl OUTPUT.json")
    with open(sys.argv[1], encoding="utf-8") as input_file:
        results = convert(input_file.readlines())
    with open(sys.argv[2], "w", encoding="utf-8") as output_file:
        json.dump(results, output_file)
        output_file.write("\n")


if __name__ == "__main__":
    main()
