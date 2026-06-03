# Spec 04 — UFA/RFA Discount Caps (bugfix)

**Rules ref:** §16.2, §18 · **Status:** 🔴 bug — discount math exists but omits max caps
**Priority:** P1 (correctness bug in existing code) · **Effort:** small

## Problem

`entity/src/entities/contract/free_agent_extension.rs:80-90` computes re-sign discounts as:

```rust
fn get_salary_discounted_by_10_percent(salary: i16) -> i16 {
    let discount_amount_rounded_up = (f32::from(salary) * 0.1).ceil();
    let discounted_salary = salary - (discount_amount_rounded_up as i16);
    cmp::max(discounted_salary, 1)
}
// _by_20_percent is identical with 0.2
```

This honors the **min $1 salary** floor but **ignores the max-discount caps** the rules require:

- **5-year UFA (20%)**: discount capped at **$8** (§16.2.1, §18).
- **3-year UFA (10%)**: discount capped at **$5** (§16.2.2, §18).
- **RFA (10%)**: per §15.2 / §18 the RFA discount has **no max cap** — only the $1 floor.

So the two helpers are *not* interchangeable across call sites: the UFA-20 and UFA-10 paths need
caps; the RFA path must not. Today all three reuse the two uncapped helpers
(`free_agent_extension.rs:44-54`).

### Worked examples (from §16.2.1–.2)

- Anthony Edwards 5yr UFA, final bid $34 → discount `ceil(.20*34)=7` (under $8 cap) → $27. ✅ today.
- A 5yr UFA with final bid $60 → uncapped discount `ceil(.20*60)=12`, but **rules cap at $8** → salary should be $52, not $48. ❌ today.
- A 3yr UFA with final bid $80 → uncapped `ceil(.10*80)=8`, **capped at $5** → salary $75, not $72. ❌ today.

## Backend changes

### `entity/`

1. Introduce a discount helper that takes an explicit max cap, e.g.:
   ```rust
   fn discounted_salary(final_bid: i16, rate: f32, max_discount: Option<i16>) -> i16 {
       let mut discount = (f32::from(final_bid) * rate).ceil() as i16;
       if let Some(cap) = max_discount { discount = discount.min(cap); }
       cmp::max(final_bid - discount, 1)
   }
   ```
2. Wire `sign_rfa_or_ufa_contract_to_team` (`free_agent_extension.rs:42-57`):
   - `UnrestrictedFreeAgentVeteran` (5-year): `discounted_salary(bid, 0.20, Some(8))`.
   - `UnrestrictedFreeAgentOriginalTeam` (3-year): `discounted_salary(bid, 0.10, Some(5))`.
   - `RestrictedFreeAgent` (4th-year re-sign): `discounted_salary(bid, 0.10, None)` **plus** the
     §15.2/§17 floor of the standard 4th-year salary (3rd-year + 20% increase) — the re-sign can
     never go *below* the normal kept salary. Confirm whether that floor is enforced anywhere; if
     not, add it here.
3. **Verify the kind→year mapping.** Current code maps `UnrestrictedFreeAgentOriginalTeam` → 10%
   and `UnrestrictedFreeAgentVeteran` → 20%. Cross-check against `ContractKind` semantics: the
   rules say *5-year* (came via Rookie→RFA→re-sign) = 20%, *3-year* (came via Veteran Auction) = 10%.
   Make sure the enum variant names line up with those origins (this is the most likely place for a
   silent inversion bug).

### Tests

- Extend the existing tests in `free_agent_extension.rs:92+` with cap-boundary cases:
  high final bids that exceed the $8/$5 caps, and an RFA case proving *no* cap is applied.
- Add the §16 worked examples ($34→$27, $13→$11) plus the new over-cap cases as regression tests.

## Frontend changes

None required for the fix itself. When the auction/RFA UI (specs 01, 03) shows a projected
re-sign salary, it should call a backend-computed value (not re-implement the math) so the cap
logic lives in one place.

## Acceptance criteria

- 5-year UFA discount never exceeds $8; 3-year UFA never exceeds $5; RFA uncapped; all floored at $1.
- RFA re-sign never below standard 4th-year salary.
- Enum-variant → discount-rate mapping verified against rules origin semantics.

## Dependencies

Standalone. Should land before [spec 01](01-live-auction-engine.md) / [spec 03](03-rfa-resolution-and-compensation.md) rely on it.
