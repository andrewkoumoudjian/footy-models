#!/usr/bin/env python3
"""Complete-everyone FM26 loader: union of all binary-derived exports, no pre-filter.

The gender-partitioned men extract (563,382 rows) is a strict UID subset of
the complete v6 binary frame (661,364 rows): 0 men UIDs are absent from
complete, 97,720 complete UIDs are absent from men. Neither file alone is
complete EITHER:

- men has attribute blocks + attr_source/height_source provenance for its
  rows, but misses 97,720 people entirely;
- complete has 97,720 extra people (27,172 with valid 1-20 traits, 41,197
  with name+DOB) but leaves attributes EMPTY where men has values (~61% of
  intersecting rows for Finishing) and ships no attr_source column.

This module loads the UNION, UID-keyed, with no CA/kind/gender pre-filter:

- every UID in either file is admitted for *identity discovery* (including
  low-CA youth, staff-kind, nonplayer-kind, women/unknown-gender rows);
- duplicate UIDs within either file are quarantined (never last-row-wins);
- per-field attribute union: 0/empty is missing; both sides in 1-20 but
  different is a per-field conflict (men value used, conflict flagged);
- kind/attr_source/height_source provenance comes from the men file when the
  UID is unique there, else `unknown_no_men_provenance` (never synthesized
  as observed db);
- file order is never trusted: all output is UID-keyed, deterministic.

Staff-kind rows are NOT dropped: a typed Reep fm:player bridge plus a valid
stored block can rescue them downstream (see fm_features recovered_db).
Kind filtering, if any, happens at the trait-admission gate, not here.
"""
from __future__ import annotations

import csv
import math
import re
import unicodedata
from collections import defaultdict
from pathlib import Path

BASE = Path("/home/drewclaw/fm26-data")
COMPLETE = BASE / "fm26_complete_v6.csv"
MEN = BASE / "fm26_men_full_attrs.csv"


def _number(value):
    if value is None or value == "":
        return None
    try:
        number = float(value)
    except (TypeError, ValueError):
        return None
    if not math.isfinite(number):
        return None
    return number


def _valid_attr(value):
    number = _number(value)
    return number if number is not None and 1.0 <= number <= 20.0 else None


def _legal_key(name):
    """Sorted normalized tokens for exact whole-legal-name matching (local copy
    to avoid an import cycle with the roster matcher)."""
    text = (name or "").casefold().translate(str.maketrans({
        "ø": "o", "ł": "l", "đ": "dj", "ð": "d", "þ": "th", "æ": "ae", "œ": "oe",
        "ß": "ss", "ı": "i", "ħ": "h", "ŧ": "t", "ŋ": "n",
    }))
    text = re.sub(r"(?<=[a-z])['’](?=[a-z])", "", text)
    text = unicodedata.normalize("NFKD", text)
    text = "".join(ch for ch in text if not unicodedata.combining(ch))
    return " ".join(sorted(re.findall(r"[a-z0-9]+", text)))


def _read(path):
    """Raw rows keyed by UID plus within-file duplicate UID set. No filtering."""
    by_uid: dict[str, list[dict]] = defaultdict(list)
    with path.open(encoding="utf-8-sig", newline="") as handle:
        for row in csv.DictReader(handle):
            uid = str(row.get("uid") or "")
            if not uid:
                continue
            by_uid[uid].append(row)
    return by_uid


def _stream(primary_path, provenance_path, wanted_uids=None, legal_targets=None):
    """One streaming pass per file. Stores only wanted UIDs; counts duplicates
    among wanted; collects legal-name hits when targets are given."""
    wanted = set(wanted_uids) if wanted_uids is not None else None
    targets = set(legal_targets) if legal_targets is not None else None
    first: dict[str, dict | None] = {}
    second: dict[str, dict | None] = {}
    dup: set[str] = set()
    counts_first: dict[str, int] = defaultdict(int)
    counts_second: dict[str, int] = defaultdict(int)
    name_hits: list[tuple] = []
    rows_primary = rows_provenance = 0
    for slot, path in (("first", primary_path), ("second", provenance_path)):
        store = first if slot == "first" else second
        counts = counts_first if slot == "first" else counts_second
        with Path(path).open(encoding="utf-8-sig", newline="") as handle:
            for row in csv.DictReader(handle):
                if slot == "first":
                    rows_primary += 1
                else:
                    rows_provenance += 1
                uid = str(row.get("uid") or "")
                if not uid:
                    continue
                if targets is not None and row.get("name"):
                    if _legal_key(row.get("name")) in targets:
                        name_hits.append((uid, row.get("name"), row.get("kind"),
                                          row.get("dob"), row.get("attr_source"), slot))
                if wanted is not None and uid not in wanted:
                    continue
                counts[uid] += 1
                if counts[uid] == 1:
                    store[uid] = row
                else:
                    dup.add(uid)
                    store.pop(uid, None)
    return first, second, dup, name_hits, rows_primary, rows_provenance


def load_union(primary_path=COMPLETE, provenance_path=MEN, wanted_uids=None,
               legal_targets=None):
    """Union of complete-everyone primary + men provenance sidecar.

    Returns (by_uid, report) where by_uid maps UID to a merged record and
    report carries quarantine/conflict counts. wanted_uids restricts stored
    output (one streaming pass per file; nothing is filtered by CA, kind, or
    gender). legal_targets optionally collects exact-legal-name hits in the
    same pass (see report["name_hits"]). File order is never trusted.
    """
    same = Path(primary_path) == Path(provenance_path)
    first, second, quarantined, name_hits, rows_primary, rows_provenance = _stream(
        primary_path, provenance_path, wanted_uids, legal_targets)
    if same:
        second = {}
    by_uid: dict[str, dict] = {}
    report = {
        "primary_rows": rows_primary,
        "provenance_rows": rows_provenance,
        "duplicate_uids_quarantined": len(quarantined),
        "complete_only_uids": 0,
        "men_only_uids": 0,
        "per_field_conflicts": 0,
        "conflict_uids": 0,
    }
    universe = set(first) | set(second)
    universe -= quarantined
    for uid in sorted(universe):
        first_row = first.get(uid)
        second_row = second.get(uid)
        if first_row is None and second_row is None:
            continue
        if first_row is None:
            report["men_only_uids"] += 1
        if second_row is None:
            report["complete_only_uids"] += 1
        # Identity fields: prefer non-empty, flag conflicts.
        name = (first_row or {}).get("name") or (second_row or {}).get("name") or ""
        dob = (first_row or {}).get("dob") or (second_row or {}).get("dob") or ""
        name_conflict = bool(first_row and second_row and first_row.get("name") and second_row.get("name")
                             and first_row.get("name") != second_row.get("name"))
        dob_conflict = bool(first_row and second_row and first_row.get("dob") and second_row.get("dob")
                            and first_row.get("dob") != second_row.get("dob"))
        kind_primary = (first_row or {}).get("kind", "unknown")
        kind_prov = (second_row or {}).get("kind", "unknown")
        kind = kind_primary
        kind_conflict = bool(first_row and second_row and kind_primary and kind_prov
                             and kind_primary != kind_prov)
        attr_source = (second_row or {}).get("attr_source") or "unknown_no_men_provenance"
        # Per-field attribute union across both frames. 0/empty is missing.
        # On a true value conflict the men side wins (it carries explicit
        # attr_source provenance and is the newer extract); the conflict is
        # recorded in per_field_conflicts, never silently resolved.
        attributes: dict[str, float | None] = {}
        conflicts: list[str] = []
        # NOTE: the men extract's provenance column `attr_source` also starts
        # with `attr_` but is NOT a 1-20 trait; it must never enter the trait
        # union (its stripped name `source` would collide downstream).
        keys = {k.removeprefix("attr_") for row in (first_row, second_row) if row
                for k in row if k.startswith("attr_") and k != "attr_source"}
        men_prov = (second_row or {}).get("attr_source") or ""
        for field in keys:
            left = _valid_attr((first_row or {}).get("attr_" + field))
            right = _valid_attr((second_row or {}).get("attr_" + field))
            if left is not None and right is not None and left != right:
                conflicts.append(field)
                attributes[field] = right if men_prov else left
            else:
                attributes[field] = left if left is not None else right
        if conflicts:
            report["per_field_conflicts"] += len(conflicts)
            report["conflict_uids"] += 1
        by_uid[uid] = {
            "uid": uid,
            "name": name,
            "dob": dob,
            "gender": (first_row or {}).get("gender", ""),
            "kind": kind,
            "kind_conflict": kind_conflict,
            "name_conflict": name_conflict,
            "dob_conflict": dob_conflict,
            "attr_source": attr_source,
            "attr_source_provenance": ("men_verified" if second_row is not None
                                       else "complete_only_unknown"),
            "ca": _number((first_row or {}).get("ca")) if _number((first_row or {}).get("ca")) is not None and 1.0 <= float((first_row or {}).get("ca")) <= 200.0
                  else (_number((second_row or {}).get("ca")) if _number((second_row or {}).get("ca")) is not None and 1.0 <= float((second_row or {}).get("ca")) <= 200.0 else None),
            "pa": _number((first_row or {}).get("pa")) if _number((first_row or {}).get("pa")) is not None and 1.0 <= float((first_row or {}).get("pa")) <= 200.0
                  else (_number((second_row or {}).get("pa")) if _number((second_row or {}).get("pa")) is not None and 1.0 <= float((second_row or {}).get("pa")) <= 200.0 else None),
            "height_cm": None,  # filled below with plausible range
            "height_raw_primary": (first_row or {}).get("height"),
            "height_raw_provenance": (second_row or {}).get("height"),
            "attributes": attributes,
            "per_field_conflicts": conflicts,
            "binary_record_source": ("complete_and_men_union" if first_row is not None and second_row is not None
                                    else "complete_v6_uid_exact" if first_row is not None else "men_full_attrs"),
        }
        for raw in (by_uid[uid]["height_raw_primary"], by_uid[uid]["height_raw_provenance"]):
            height = _number(raw)
            if height is not None and 140.0 <= height <= 220.0:
                by_uid[uid]["height_cm"] = height
                break
    report["union_uids"] = len(by_uid)
    report["name_hits"] = name_hits
    return by_uid, report


def name_index(by_uid):
    """Every named row, no CA/kind/gender filter, for exact-legal-name search."""
    index: dict[str, list[dict]] = defaultdict(list)
    for uid, record in by_uid.items():
        if record.get("name"):
            index[record["name"]].append(record)
    return index
