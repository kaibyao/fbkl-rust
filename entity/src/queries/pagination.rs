//! Offset pagination shared by every unbounded list query the GraphQL surface exposes
//! (the league transaction feed, auction bid history).
//!
//! The convention is `(page, page_size)` — zero-indexed page, explicit size — rather than
//! opaque cursors. The lists are league-scoped and small enough that a stable `ORDER BY` plus
//! an offset is correct, and the frontend needs `total_items` to render page controls anyway.

use color_eyre::Result;
use sea_orm::{ConnectionTrait, EntityTrait, PaginatorTrait, Select};

/// One page of rows plus the total row count for the unpaginated query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paged<T> {
    pub items: Vec<T>,
    pub total_items: u64,
}

/// Runs `query` as a single page. Order the query before calling — offset paging over an
/// unordered select returns arbitrary rows.
pub async fn fetch_page<E, C>(
    query: Select<E>,
    page: u64,
    page_size: u64,
    db: &C,
) -> Result<Paged<E::Model>>
where
    E: EntityTrait,
    E::Model: Send + Sync,
    C: ConnectionTrait,
{
    let paginator = query.paginate(db, page_size.max(1));
    let total_items = paginator.num_items().await?;
    let items = paginator.fetch_page(page).await?;

    Ok(Paged { items, total_items })
}
