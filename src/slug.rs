#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::repos::RepoError;

/// Maximum parent-chain depth before we assume data corruption.
const MAX_DEPTH: usize = 64;

/// Minimal component data for slug resolution.
#[derive(Debug, Clone)]
struct CachedComponent {
    parent_id: Option<i64>,
    slug: Option<String>,
}

/// Thread-safe in-memory cache for resolving effective component slugs.
///
/// The "effective slug" for a component is the slug of the nearest ancestor
/// (including itself) that has a non-null slug.  This lets child components
/// inherit human-readable prefixes like `NET-42` without storing slugs on
/// every node.
#[derive(Debug, Clone)]
pub struct SlugCache {
    inner: Arc<RwLock<HashMap<i64, CachedComponent>>>,
}

impl SlugCache {
    /// Build the cache by loading all components from the database.
    pub async fn new(pool: &SqlitePool) -> Result<Self, RepoError> {
        let map = load_from_db(pool).await?;
        Ok(Self {
            inner: Arc::new(RwLock::new(map)),
        })
    }

    /// Reload the entire cache after a component mutation.
    ///
    /// The query runs before the write lock is acquired so readers are not
    /// blocked during the database round-trip.
    pub async fn reload(&self, pool: &SqlitePool) -> Result<(), RepoError> {
        let map = load_from_db(pool).await?;
        let mut guard = self.inner.write().await;
        *guard = map;
        Ok(())
    }

    /// Resolve the effective slug for `component_id` by walking up the
    /// parent chain to the first non-null slug.
    ///
    /// Returns `NotFound` if the component ID is unknown and `Conflict` if the
    /// chain reaches a root node that has no slug (data integrity issue).
    pub async fn resolve_effective_slug(&self, component_id: i64) -> Result<String, RepoError> {
        let guard = self.inner.read().await;
        walk_slug(&guard, component_id)
    }

    /// Format a human-readable ticket identifier: `{effective_slug}-{ticket_id}`.
    pub async fn ticket_slug(
        &self,
        component_id: i64,
        ticket_id: i64,
    ) -> Result<String, RepoError> {
        let slug = self.resolve_effective_slug(component_id).await?;
        Ok(format!("{slug}-{ticket_id}"))
    }

    /// Batch-resolve effective slugs for multiple component IDs.
    ///
    /// Acquires the read lock once for the entire batch.  Returns an error if
    /// any ID is unknown or reaches a root without a slug.
    pub async fn resolve_many(
        &self,
        component_ids: &[i64],
    ) -> Result<HashMap<i64, String>, RepoError> {
        let guard = self.inner.read().await;
        let mut out = HashMap::with_capacity(component_ids.len());
        for &id in component_ids {
            out.insert(id, walk_slug(&guard, id)?);
        }
        Ok(out)
    }
}

/// Load `(id, parent_id, slug)` for every component.
async fn load_from_db(pool: &SqlitePool) -> Result<HashMap<i64, CachedComponent>, RepoError> {
    let rows: Vec<(i64, Option<i64>, Option<String>)> =
        sqlx::query_as("SELECT id, parent_id, slug FROM components")
            .fetch_all(pool)
            .await?;

    let mut map = HashMap::with_capacity(rows.len());
    for (id, parent_id, slug) in rows {
        map.insert(id, CachedComponent { parent_id, slug });
    }
    Ok(map)
}

/// Walk the parent chain starting at `component_id` until a non-null slug is
/// found.  Stops after `MAX_DEPTH` hops to guard against corrupted cycles.
fn walk_slug(map: &HashMap<i64, CachedComponent>, component_id: i64) -> Result<String, RepoError> {
    let mut current = component_id;
    for _ in 0..MAX_DEPTH {
        let entry = map.get(&current).ok_or(RepoError::NotFound)?;
        if let Some(ref slug) = entry.slug {
            return Ok(slug.clone());
        }
        match entry.parent_id {
            Some(pid) => current = pid,
            None => {
                return Err(RepoError::Conflict(format!(
                    "root component {current} has no slug"
                )));
            }
        }
    }
    Err(RepoError::Conflict(format!(
        "slug resolution exceeded {MAX_DEPTH} levels for component {component_id}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::{CreateComponentRequest, CreateUserRequest};
    use crate::repos::{component, user};

    async fn test_pool() -> SqlitePool {
        let pool = db::init_memory_pool().await.unwrap();
        db::run_migrations(&pool).await.unwrap();
        pool
    }

    async fn seed_user(pool: &SqlitePool) -> i64 {
        let req = CreateUserRequest {
            login: "slugowner".to_string(),
            display_name: "Slug Owner".to_string(),
            email: "slug@test.com".to_string(),
            password: None,
            role: None,
        };
        user::create(pool, &req, None).await.unwrap().id
    }

    /// Insert a component and return its id.
    async fn insert(
        pool: &SqlitePool,
        name: &str,
        parent_id: Option<i64>,
        slug: Option<&str>,
        owner_id: i64,
    ) -> i64 {
        component::create(
            pool,
            &CreateComponentRequest {
                name: name.to_string(),
                parent_id,
                slug: slug.map(|s| s.to_string()),
                owner_id,
            },
        )
        .await
        .unwrap()
        .id
    }

    // ── resolve: own slug ────────────────────────────────────────────────

    #[tokio::test]
    async fn resolve_root_own_slug() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let id = insert(&pool, "Platform", None, Some("PLAT"), owner).await;

        let cache = SlugCache::new(&pool).await.unwrap();
        assert_eq!(cache.resolve_effective_slug(id).await.unwrap(), "PLAT");
    }

    #[tokio::test]
    async fn resolve_child_own_slug() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let plat = insert(&pool, "Platform", None, Some("PLAT"), owner).await;
        let net = insert(&pool, "Networking", Some(plat), Some("NET"), owner).await;

        let cache = SlugCache::new(&pool).await.unwrap();
        assert_eq!(cache.resolve_effective_slug(net).await.unwrap(), "NET");
    }

    // ── resolve: inherited slug ──────────────────────────────────────────

    #[tokio::test]
    async fn resolve_child_inherits_parent() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let plat = insert(&pool, "Platform", None, Some("PLAT"), owner).await;
        let storage = insert(&pool, "Storage", Some(plat), None, owner).await;

        let cache = SlugCache::new(&pool).await.unwrap();
        assert_eq!(cache.resolve_effective_slug(storage).await.unwrap(), "PLAT");
    }

    #[tokio::test]
    async fn resolve_grandchild_two_levels() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let plat = insert(&pool, "Platform", None, Some("PLAT"), owner).await;
        let net = insert(&pool, "Networking", Some(plat), None, owner).await;
        let dns = insert(&pool, "DNS", Some(net), None, owner).await;

        let cache = SlugCache::new(&pool).await.unwrap();
        assert_eq!(cache.resolve_effective_slug(dns).await.unwrap(), "PLAT");
    }

    #[tokio::test]
    async fn resolve_grandchild_stops_at_middle() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let plat = insert(&pool, "Platform", None, Some("PLAT"), owner).await;
        let net = insert(&pool, "Networking", Some(plat), Some("NET"), owner).await;
        let dns = insert(&pool, "DNS", Some(net), None, owner).await;

        let cache = SlugCache::new(&pool).await.unwrap();
        assert_eq!(cache.resolve_effective_slug(dns).await.unwrap(), "NET");
    }

    // ── resolve: error cases ─────────────────────────────────────────────

    #[tokio::test]
    async fn resolve_unknown_not_found() {
        let pool = test_pool().await;
        let cache = SlugCache::new(&pool).await.unwrap();
        assert!(matches!(
            cache.resolve_effective_slug(9999).await,
            Err(RepoError::NotFound)
        ));
    }

    #[tokio::test]
    async fn empty_cache() {
        let pool = test_pool().await;
        let cache = SlugCache::new(&pool).await.unwrap();
        assert!(matches!(
            cache.resolve_effective_slug(1).await,
            Err(RepoError::NotFound)
        ));
    }

    // ── ticket_slug ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn ticket_slug_format() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let id = insert(&pool, "Platform", None, Some("PLAT"), owner).await;

        let cache = SlugCache::new(&pool).await.unwrap();
        assert_eq!(cache.ticket_slug(id, 42).await.unwrap(), "PLAT-42");
    }

    #[tokio::test]
    async fn ticket_slug_inherited() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let plat = insert(&pool, "Platform", None, Some("PLAT"), owner).await;
        let storage = insert(&pool, "Storage", Some(plat), None, owner).await;

        let cache = SlugCache::new(&pool).await.unwrap();
        assert_eq!(cache.ticket_slug(storage, 7).await.unwrap(), "PLAT-7");
    }

    // ── reload ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn reload_picks_up_new_component() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;

        let cache = SlugCache::new(&pool).await.unwrap();

        // Add a component after cache init.
        let id = insert(&pool, "Platform", None, Some("PLAT"), owner).await;
        assert!(cache.resolve_effective_slug(id).await.is_err());

        cache.reload(&pool).await.unwrap();
        assert_eq!(cache.resolve_effective_slug(id).await.unwrap(), "PLAT");
    }

    #[tokio::test]
    async fn reload_picks_up_updated_slug() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let id = insert(&pool, "Platform", None, Some("PLAT"), owner).await;

        let cache = SlugCache::new(&pool).await.unwrap();
        assert_eq!(cache.resolve_effective_slug(id).await.unwrap(), "PLAT");

        // Update slug directly in DB.
        sqlx::query("UPDATE components SET slug = 'CORE' WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();

        // Stale value before reload.
        assert_eq!(cache.resolve_effective_slug(id).await.unwrap(), "PLAT");

        cache.reload(&pool).await.unwrap();
        assert_eq!(cache.resolve_effective_slug(id).await.unwrap(), "CORE");
    }

    // ── resolve_many ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn resolve_many_batch() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let plat = insert(&pool, "Platform", None, Some("PLAT"), owner).await;
        let net = insert(&pool, "Networking", Some(plat), Some("NET"), owner).await;
        let storage = insert(&pool, "Storage", Some(plat), None, owner).await;

        let cache = SlugCache::new(&pool).await.unwrap();
        let result = cache.resolve_many(&[plat, net, storage]).await.unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[&plat], "PLAT");
        assert_eq!(result[&net], "NET");
        assert_eq!(result[&storage], "PLAT");
    }

    #[tokio::test]
    async fn resolve_many_with_unknown_fails() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let plat = insert(&pool, "Platform", None, Some("PLAT"), owner).await;

        let cache = SlugCache::new(&pool).await.unwrap();
        let result = cache.resolve_many(&[plat, 9999]).await;
        assert!(matches!(result, Err(RepoError::NotFound)));
    }
}
