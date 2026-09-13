#!/usr/bin/env python3

# Copyright (c) 2019-2026 Provable Inc.
# This file is part of the snarkVM library.

# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at:

# http://www.apache.org/licenses/LICENSE-2.0

# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""Checks every `.rs` file carries the licence header and balances its lock imports.

These ran from `build.rs` until they were moved here. As a build script they
declared no `cargo:rerun-if-changed`, which makes cargo fingerprint the package
by the newest mtime among its files -- so the crate rebuilt whenever anything in
the repository was touched, and the walk below ran on every build of the root
package rather than once per push.
"""

import datetime
import sys
from pathlib import Path

LICENSE = Path(".resources/license_header").read_bytes()

# The year sits at a fixed offset in the header, as `2019-YYYY`.
LICENSE_YEAR = slice(22, 26)

DIRS_TO_SKIP = {".cargo", ".circleci", ".git", ".github", "target"}


def rust_files(root):
    for path in sorted(Path(root).rglob("*.rs")):
        if DIRS_TO_SKIP.isdisjoint(part for part in path.parts):
            yield path


def check_license_year():
    """The header names a range ending in the current year; a new year needs a new header."""
    year = LICENSE[LICENSE_YEAR].decode()
    current = str(datetime.date.today().year)
    if year != current:
        return [f".resources/license_header: says {year}, the current year is {current}"]
    return []


def check_licenses(paths):
    return [
        f"{path}: the licence header is missing or does not match .resources/license_header"
        for path in paths
        if path.read_bytes()[: len(LICENSE)] != LICENSE
    ]


def check_locktick_imports(paths):
    """Every `parking_lot`/`tokio` lock import needs a `locktick` counterpart.

    Counted rather than matched by name: the import block is scanned and each
    lock type adds one, each `locktick` lock type takes one away, and anything
    left over is a lock whose counterpart is missing. Guards are subtracted
    because `locktick` has none, and `TMutex` because `tokio::Mutex as TMutex`
    is the convention for the one that is deliberately not wrapped.
    """
    problems = []
    for path in paths:
        lines = path.read_text().splitlines()
        lines = [line for line in lines if line]
        # The import block: from the first `use` to the last line that still
        # looks like part of one.
        start = next((i for i, line in enumerate(lines) if line.startswith("use")), None)
        if start is None:
            continue
        block = []
        for line in lines[start:]:
            if (
                line.startswith("use")
                or line.startswith("#[cfg")
                or line.startswith("//")
                or line == "};"
                or line[:1].isspace()
            ):
                block.append(line)
            else:
                break

        interest = None
        balance = 0
        for line in block:
            if interest is None:
                for prefix, kind in (
                    ("use locktick::", "locktick"),
                    ("use parking_lot::", "wrapped"),
                    ("use tokio::", "wrapped"),
                ):
                    if line.startswith(prefix):
                        interest = kind
                        break
            if interest == "wrapped":
                balance += line.count("Mutex") + line.count("RwLock")
                if "TMutex" in line:
                    balance -= 1
                for guard in ("MutexGuard", "RwLockReadGuard", "RwLockWriteGuard"):
                    if guard in line:
                        balance -= 1
            elif interest == "locktick":
                balance -= line.count("Mutex") + line.count("RwLock")
                if "TMutex" in line:
                    balance += 1
            if line.endswith(";"):
                interest = None

        if balance != 0:
            problems.append(f"{path}: the locks here do not have `locktick` counterparts")
    return problems


def main():
    paths = list(rust_files("."))
    if not paths:
        print("no .rs files found -- run this from the repository root", file=sys.stderr)
        return 2

    problems = check_license_year() + check_licenses(paths) + check_locktick_imports(paths)
    for problem in problems:
        print(problem, file=sys.stderr)
    print(f"checked {len(paths)} files, {len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main())
