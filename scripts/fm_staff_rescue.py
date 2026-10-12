#!/usr/bin/env python3
"""Ledger-directed rescue of staff-kind FM26 db traits (identity untouched).

The canonical Reep-first ledger attaches FM identities but quarantines ALL
traits for 732 registrations -- including staff-kind rows whose db attribute
block belongs to the right person (Transfermarkt-DOB corroborated). The MLS
label pipeline already rescues exactly this category
(`fm_row_usable`: staff-kind + attr_source=db + >=1 valid attr block); the
EPL bundle gate does not, which is an indefensible double standard for the
same export artifact.

This module consumes the ledger's PUBLISHED verdicts (no re-matching, no new
identity evidence) and returns the rescuable subset under an explicit policy:

- ledger status attached-quarantined, native kind staff, native attr_source db
- native DOB agrees with Transfermarkt
- native name evidence tier >= 2 (tier 0-1 stays out: 8 rows)
- the complete-everyone union row passes the same fm_row_usable gate the MLS
  labels use (right person, real db block, >=1 valid attr)

Excluded by construction: absent-from-export rows (nothing to use),
DOB-conflict rows (wrong person), derived/unknown provenance (prior-grade,
not observed), and the 5 player-kind rows the resolver gated for other
reasons (respected, not overridden). Identity verdicts are never altered;
callers record `fm_rescued_staff` provenance per admitted player.
"""
from __future__ import annotations

import json
from pathlib import Path

RESCUE_MIN_NAME_TIER = 2


def rescue_candidate_uids(ledger_path: Path) -> set[str]:
    """FM uids meeting the ledger-side rescue policy (no union data needed).

    Used to size the complete-everyone union load before validity gating.
    """
    ledger = json.loads(ledger_path.read_text())
    players = ledger.get("players", ledger) if isinstance(ledger, dict) else ledger
    out: set[str] = set()
    for person in players:
        if person.get("status") != "candidate_uid_attached_attributes_quarantined":
            continue
        accepted = person.get("accepted_candidate") or {}
        if accepted.get("native_kind") != "staff":
            continue
        if accepted.get("native_attr_source") != "db":
            continue
        if accepted.get("native_dob_agrees_with_tm") is not True:
            continue
        if (accepted.get("native_name_evidence_tier") or 0) < RESCUE_MIN_NAME_TIER:
            continue
        if accepted.get("fm_uid") is not None:
            out.add(str(accepted.get("fm_uid")))
    return out


def load_rescue_map(ledger_path: Path, fm_usable: dict) -> dict:
    """Return {(club, official_name): info} for rescuable staff-db traits.

    `fm_usable` maps FM uid -> union row and must already enforce the
    kind/provenance/validity gate (the MLS `fm_row_usable` parity check).
    """
    from build_fm_mls_shot_dataset import fm_row_usable  # noqa: E402

    ledger = json.loads(ledger_path.read_text())
    players = ledger.get("players", ledger) if isinstance(ledger, dict) else ledger
    rescue: dict[tuple[str, str], dict] = {}
    for person in players:
        if person.get("status") != "candidate_uid_attached_attributes_quarantined":
            continue
        accepted = person.get("accepted_candidate") or {}
        if accepted.get("native_kind") != "staff":
            continue
        if accepted.get("native_attr_source") != "db":
            continue
        if accepted.get("native_dob_agrees_with_tm") is not True:
            continue
        if (accepted.get("native_name_evidence_tier") or 0) < RESCUE_MIN_NAME_TIER:
            continue
        fm_uid = str(accepted.get("fm_uid"))
        row = fm_usable.get(fm_uid)
        if row is None:
            continue
        good, rescued = fm_row_usable([dict(row)])
        if good is None or not rescued:
            # Not a valid staff-db block under MLS parity rules; skip.
            continue
        rescue[(person.get("club"), person.get("official_name"))] = {
            "fm_uid": fm_uid,
            "name_tier": accepted.get("native_name_evidence_tier"),
            "tm_club_compatible": accepted.get("tm_club_compatible"),
        }
    return rescue
