---
name: idiomatic-rust
description: Idiomatic Rust judgment rules distilled from idiomatic-rust-snippets.org, Canonical's rust-best-practices, and cheats.rs. Read BEFORE writing or reviewing any non-trivial Rust code in this repo. Covers only what clippy/rustfmt cannot check — API design, error architecture, ownership choices, trait design, module layout. Mechanical style is already enforced by clippy pedantic+nursery (workspace lints) + lefthook `-D warnings`.
---

# Idiomatic Rust — judgment rules

Clippy pedantic/nursery already enforce mechanical style (this workspace denies warnings at commit time via lefthook). These rules cover what a linter can't: design choices. Apply them when writing new Rust; flag violations when reviewing.

## Mindset

- **Think in expressions** — `let y = if cond { a } else { b }`; avoid declare-then-assign (`let x; if … { x = … }`). Compute inside the branching expression, bind once.
- **Think in iterators** — prefer iterator chains over manual index loops; they compose and avoid off-by-one bugs.
- **Model absence with `Option`/`Result` and `?`** — never sentinel values.
- **Make illegal states unrepresentable** — design types so invalid combinations cannot be constructed, instead of documenting invariants and hoping.

## Type & API design

- **Newtype primitives that carry meaning** — `struct UserId(u64)` over bare `u64`; prevents swapped-argument bugs at zero runtime cost. Also the orphan-rule workaround.
- **Enums over strings/bools for fixed-value domains** — a field with a known value set is an enum; typos become compile errors. `enum Visibility { Visible, Hidden }` over `bool`.
- **Struct for all-present field groups, enum for one-of-several shapes.**
- **Typestate for state machines** — per-state types (`Document<Draft>`, `Document<Published>`, via `PhantomData`) with state-specific methods, so illegal transitions are compile errors, not runtime flags.
- **Builder for many-optional-field construction** — `T::builder() -> TBuilder`, setters consume `self` (not `&mut self`), fallible `build() -> Result<T>`. Callers never construct `TBuilder` directly. Typestate-builder when required fields must be a compile-time guarantee.
- **Constructors/builders over public fields** for behavior-carrying structs; plain data-transfer structs with a sane `Default` may expose fields (consider `#[non_exhaustive]`).
- **No `Deref` to fake inheritance** — `Deref`/`DerefMut` only for transparent smart-pointer wrappers; composition gets explicit accessors.
- **Casting fights are signature smells** — needing `as`/transmute to line types up means the signature is wrong (return `Option`/`Result`, take a slice/array). Fix the types.

## Error handling

- **Libraries return `Result`, never panic on expected failure** — panics are for unrecoverable programmer bugs only; prefix internal-invariant panics with `internal error:`. `.unwrap()` only where failure can only mean a bug (`Regex::new(literal)`) or in tests.
- **Enumerated error types over type-erasure** — a custom enum (this repo: `FbklError` pattern, `thiserror`-style `From` impls) so callers can match on failure modes; `Box<dyn Error>` only in prototypes/binaries.
- **Convert dependency errors at the boundary you call them** — one error vocabulary internally, `?` propagates naturally. Hide a dependency's error behind an `Internal(...)` variant rather than re-exporting its type.
- **Error text**: start with a verb ("cannot …"); lowercase first letter if the message may be wrapped; don't repeat context an outer layer will add; consistent phrasing across variants.

## Ownership & smart pointers

- **Pick Box/Rc/Arc/RefCell for a specific need, not by habit** — `Box` for heap/recursion/trait objects; `Rc`/`Arc` only for genuine shared ownership; `RefCell`/`Mutex` only for real interior mutability. Don't reach for `Arc` when `Rc` suffices (atomics cost even uncontended).
- **Return owned values over lifetime juggling** — when a lifetime error tempts multi-parameter annotations or out-params, simpler ownership usually beats "technically correct" lifetimes.
- **Scope `let mut` tightly** — wrap mutation in its own block/expression so mutability can't leak.
- **RAII over manual cleanup** — resource-owning types implement `Drop`; never rely on callers remembering `close()`.

## Traits & generics

- **Associated types when one impl per type** (`Iterator::Item`); **generic params when the caller picks** (`Convert<T>`).
- **Static vs dyn dispatch deliberately** — generics/`impl Trait` for zero-cost known-at-compile-time; `Box<dyn Trait>` for runtime-swappable/heterogeneous/plugin. Neither is the default.
- **Generics in moderation** — `<T: AsRef<Path>>` makes an API nicer; bound soup makes it unreadable. Hide params callers don't need (`impl Trait`, elision).
- **Split impls by concrete type** — shared `impl<T> Point<T>` + separate `impl Point<f32>`, not one generic impl with runtime branching.
- **Extension traits** to add methods to foreign types; only for genuinely reusable helpers.
- **Accept the least restrictive closure trait** — `FnOnce` easiest for callers; don't require `Fn` because it "feels safest".
- **Don't trust generic callers' trait impls** — third-party `PartialEq`/`Hash`/`Ord`/`Drop` can be adversarial (panic, self-inequality); generic code needing real cooperation requires an `unsafe trait`.

## API stability (semver judgment)

- New enum variant / new field on a variant = breaking unless `#[non_exhaustive]` from the start.
- Non-defaulted trait item breaks every existing impl; even defaulted items or new inherent methods can break method resolution.
- Bounds only loosen safely, never tighten. Adding a public field breaks if no private field existed (exhaustive destructuring).

## Naming & structure

- **Name for meaning, not mechanism** — `is_in(haystack, needle)` not `is_in(a, b)`; one word per concept API-wide; consistent verb_noun order. An unexplainable name signals a design problem — refactor, don't rename.
- **Pattern-matched bindings reuse the source name**; limit shadowing to one level per scope, same type (exception: deliberate self-consuming transform).
- **Don't index tuples positionally** past one-field newtypes — destructure into named bindings.
- **Unpack params on the body's first line**, not in the signature (closures excepted).
- **Blank lines mark conceptual grouping** — none between a declaration and its sole using block; don't interleave unrelated logic.
- **Scope single-use helper types inside the fn** under a marker comment, not module scope.

## Modules & files

- `mod.rs`/`lib.rs` declare structure only — no definitions; order `pub mod` → `pub(crate) mod` → `mod` → `pub use` → `use` (this repo uses `foo/mod.rs` style — keep it).
- Order files like a tour: public items first, helpers below; a type's impls live with the type.
- Library crates: `Error`/`Result` in `lib.rs` after imports; binaries: `error.rs`.

## Docs

- First sentence (≤2 lines) says **when/why to use** the item, not a restatement of its mechanism. Then blank line, then detail + `# Examples` for public APIs.
- Refer to params by real name ("the `deadline`"), never "a given value".

## Concurrency & async

- **Channels over `Arc<Mutex<T>>`** when components just need to communicate — automatic cleanup, no lock contention.
- **Async for I/O-bound, threads for CPU-bound**; never block the executor thread inside `async` (sync sleep/IO stalls the whole worker).
- **State may not survive `.await`** — may resume on another thread (thread-locals invalid) or never resume (future dropped); drop-guard invariants that must restore.
- **Non-`Send` captures (`Rc`) across `.await` poison the future's `Send`-ness.**
- **Globals via `OnceLock`/`LazyLock`** (std), never `static mut`; avoid global state where explicit passing works.

## Performance (judgment, post-profile)

- Reserve collection capacity when size is knowable; buffer I/O explicitly.
- Design batch APIs (slices/iterators) over one-item-at-a-time.
- Locks/atomics only where actually needed on hot paths; cache-locality layouts only once profiling justifies.

## Unsafe

- Last resort — an unmet safety guarantee, never a hoped-for speedup (`.get()` is usually as fast as unchecked).
- Minimize block scope (three 1-line blocks beat one wrapped fn); every block gets a real `// SAFETY:` stating the precondition relied on.
- Never expose an unsound safe abstraction — if the invariant can't be fully encapsulated, don't build it.

## Pattern translations (when the shape fits, not by default)

- Closed variant set + operations → **enum + match**, not Visitor traits (exhaustiveness, no vtables).
- Varies-by-config/test construction → **factory trait returning `Box<dyn Trait>`** (DI, mocking).
- Two independent varying dimensions → **two traits composed (bridge)**, not one type per combination.
- Incompatible external interface → **adapter wrapper** implementing your trait.
- Middleware pipelines → **chain of responsibility**; N-to-N component chatter → **mediator**; shared immutable data → **flyweight (`Arc` + keyed cache)**; cross-cutting behavior → **proxy wrapper**, same trait; multi-call coordination → **facade**.

Sources: idiomatic-rust-snippets.org · canonical.github.io/rust-best-practices · cheats.rs (Coding Guides). Re-distill if those sites materially change.
