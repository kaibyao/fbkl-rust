# Spec 12 — Out-of-Scope & External Systems

**Rules ref:** §1–2 · **Status:** ⚪ external · **Priority:** reference only

This is **not** a build spec. It records which rules are owned by external systems so they are
not mistaken for missing FBKL features — and flags the thin slice of derived data FBKL *does* need
to ingest from those systems.

## Owned by Fantrax (league website) — do not build

| Rule | Why external |
|------|--------------|
| §1.2 Scoring (H2H, 9 categories, weekly matchups) | Fantrax computes matchups and category wins. |
| §1.3 Lineups, semi-weekly periods, position eligibility enforcement | Fantrax runs lineups; position eligibility is determined by the league website. |
| §1.4 Playoff bracket play, byes, weekly playoff matchups | Played on Fantrax. |
| §2.1 Roster-lock *timing* (Monday tipoff) | Fantrax defines the lock moment; FBKL mirrors it as a `deadline`. |
| §10 IR eligibility (on NBA IR?) | Determined by the league website. |

## Owned by humans / commissioner — not software

| Rule | Notes |
|------|-------|
| §2.2 Buy-in / prize-pool payouts | Real money, commissioner-tracked. No entity planned (could add a ledger later if desired). |
| §2.3 Rules-change voting | Manual league process. |
| §3.1 Commissioner discretion calls (NBA-roster classification, RDI eligibility, collusion vetoes) | Human judgment; software should *allow override* but not decide. |
| §12.6 Collusion enforcement | Commissioner discretion; no automated veto (rules explicitly say no league voting). |

## External communication

| Rule | Notes |
|------|-------|
| §2.1 Google Group correspondence | Email out is a `server/src/main.rs` TODO (SendGrid). FA/trade *report* generation could be a future feature but the discussion channel itself is external. |

## ⚠️ Derived data FBKL MUST ingest (not fully external)

These come *from* Fantrax/NBA but are **required inputs** to FBKL rules engines — they are real work,
tracked in other specs:

1. **Final regular-season standings + the "≈2/3 season" standings snapshot** — drive rookie-draft
   order and the lottery odds. See [spec 02](02-rookie-draft-engine.md). Without ingesting these,
   the draft engine cannot compute order.
2. **Playoff finish (who lost which round)** — breaks draft-order ties among playoff teams. Spec 02.
3. **NBA roster / "has been on an active NBA roster" status** — drives auction-vs-draft eligibility
   and RDI eligibility. See [spec 10](10-eligibility-and-player-pool.md). Partly importable from
   basketball-reference-style data; commissioner override for edge cases.
4. **NBA IR status** — gates IR-slot eligibility. Likely a synced flag on `league_player`/player.

**Takeaway:** "Fantrax owns scoring" does **not** mean FBKL needs no integration. The standings,
playoff results, and NBA-roster/IR flags are hard dependencies for the draft, eligibility, and IR
features — design an ingestion path (manual entry + optional API sync) for them.
