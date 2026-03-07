use chrono::Utc;
use sqlx::SqlitePool;

use crate::models::{ComponentRow, CreateComponentRequest, UpdateComponentRequest};

use super::RepoError;

/// Validates a slug against `^[A-Z][A-Z0-9]{1,9}$` (2–10 uppercase chars, starts with letter).
fn validate_slug(slug: &str) -> Result<(), RepoError> {
    let len = slug.len();
    if len < 2 || len > 10 {
        return Err(RepoError::Conflict(format!(
            "slug must be 2–10 characters, got {len}"
        )));
    }
    let bytes = slug.as_bytes();
    if !bytes[0].is_ascii_uppercase() {
        return Err(RepoError::Conflict(
            "slug must start with an uppercase letter".to_string(),
        ));
    }
    if !bytes[1..].iter().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
        return Err(RepoError::Conflict(
            "slug must contain only uppercase letters and digits".to_string(),
        ));
    }
    Ok(())
}

/// Returns all components ordered by materialized path.
pub async fn list(pool: &SqlitePool) -> Result<Vec<ComponentRow>, RepoError> {
    let rows = sqlx::query_as::<_, ComponentRow>("SELECT * FROM components ORDER BY path")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// Finds a component by primary key.
pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Option<ComponentRow>, RepoError> {
    let row = sqlx::query_as::<_, ComponentRow>("SELECT * FROM components WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Returns the total number of components.
pub async fn count(pool: &SqlitePool) -> Result<i64, RepoError> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM components")
        .fetch_one(pool)
        .await?;
    Ok(n)
}

/// Returns direct children of a component. Pass `None` to get root components.
pub async fn get_children(
    pool: &SqlitePool,
    parent_id: Option<i64>,
) -> Result<Vec<ComponentRow>, RepoError> {
    let rows = match parent_id {
        Some(pid) => {
            sqlx::query_as::<_, ComponentRow>(
                "SELECT * FROM components WHERE parent_id = ? ORDER BY name",
            )
            .bind(pid)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query_as::<_, ComponentRow>(
                "SELECT * FROM components WHERE parent_id IS NULL ORDER BY name",
            )
            .fetch_all(pool)
            .await?
        }
    };
    Ok(rows)
}

/// Returns all components under a path prefix (including the node itself).
pub async fn get_subtree(
    pool: &SqlitePool,
    path_prefix: &str,
) -> Result<Vec<ComponentRow>, RepoError> {
    let rows = sqlx::query_as::<_, ComponentRow>(
        "SELECT * FROM components WHERE path LIKE ? ORDER BY path",
    )
    .bind(format!("{path_prefix}%"))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Creates a new component, computing its materialized path from the parent.
///
/// Root components (no parent) must have a slug. Child components may omit it.
pub async fn create(
    pool: &SqlitePool,
    req: &CreateComponentRequest,
) -> Result<ComponentRow, RepoError> {
    if let Some(ref slug) = req.slug {
        validate_slug(slug)?;
    }

    if req.parent_id.is_none() && req.slug.is_none() {
        return Err(RepoError::Conflict(
            "root components must have a slug".to_string(),
        ));
    }

    let path = match req.parent_id {
        None => format!("/{}/", req.name),
        Some(pid) => {
            let parent = get_by_id(pool, pid)
                .await?
                .ok_or(RepoError::NotFound)?;
            format!("{}{}/", parent.path, req.name)
        }
    };

    let now = Utc::now();
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO components (name, parent_id, path, slug, owner_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(&req.name)
    .bind(req.parent_id)
    .bind(&path)
    .bind(&req.slug)
    .bind(req.owner_id)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;

    Ok(get_by_id(pool, id).await?.unwrap())
}

/// Updates a component with transactional reparent and descendant path cascade.
///
/// Uses a transaction to atomically: update the component row, detect circular
/// references, and cascade path changes to all descendants. Slug uses double-Option:
/// `None` = keep, `Some(None)` = clear, `Some(Some(v))` = set.
pub async fn update(
    pool: &SqlitePool,
    id: i64,
    req: &UpdateComponentRequest,
) -> Result<ComponentRow, RepoError> {
    let mut tx = pool.begin().await?;

    let existing =
        sqlx::query_as::<_, ComponentRow>("SELECT * FROM components WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(RepoError::NotFound)?;

    let name = req.name.as_deref().unwrap_or(&existing.name);
    let owner_id = req.owner_id.unwrap_or(existing.owner_id);

    // Resolve double-Option parent_id: None = keep, Some(None) = root, Some(Some(v)) = set
    let new_parent_id = match &req.parent_id {
        None => existing.parent_id,
        Some(inner) => *inner,
    };

    // Resolve double-Option slug: None = keep, Some(None) = clear, Some(Some(v)) = set
    let new_slug = match &req.slug {
        None => existing.slug.clone(),
        Some(inner) => inner.clone(),
    };

    if let Some(ref slug) = new_slug {
        validate_slug(slug)?;
    }

    // Root components must have a slug
    if new_parent_id.is_none() && new_slug.is_none() {
        return Err(RepoError::Conflict(
            "root components must have a slug".to_string(),
        ));
    }

    // Compute new path
    let new_path = match new_parent_id {
        None => format!("/{name}/"),
        Some(pid) => {
            let parent =
                sqlx::query_as::<_, ComponentRow>("SELECT * FROM components WHERE id = ?")
                    .bind(pid)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or(RepoError::NotFound)?;

            // Circular reference check: new parent must not be under this node
            if parent.path.starts_with(&existing.path) {
                return Err(RepoError::Conflict(
                    "cannot move component under its own descendant".to_string(),
                ));
            }

            format!("{}{name}/", parent.path)
        }
    };

    let now = Utc::now();
    sqlx::query(
        "UPDATE components SET name = ?, parent_id = ?, path = ?, slug = ?, owner_id = ?, updated_at = ? WHERE id = ?",
    )
    .bind(name)
    .bind(new_parent_id)
    .bind(&new_path)
    .bind(&new_slug)
    .bind(owner_id)
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    // Cascade path changes to descendants
    let old_path = &existing.path;
    if new_path != *old_path {
        sqlx::query(
            "UPDATE components SET path = ?1 || substr(path, length(?2) + 1), updated_at = ?3
             WHERE path LIKE ?4 AND id != ?5",
        )
        .bind(&new_path)
        .bind(old_path)
        .bind(now)
        .bind(format!("{old_path}%"))
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(get_by_id(pool, id).await?.unwrap())
}

/// Deletes a component. Rejects if it has children or tickets referencing it.
pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), RepoError> {
    // Verify existence
    get_by_id(pool, id).await?.ok_or(RepoError::NotFound)?;

    let (child_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM components WHERE parent_id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?;
    if child_count > 0 {
        return Err(RepoError::Conflict(
            "cannot delete component with children".to_string(),
        ));
    }

    let (ticket_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM tickets WHERE component_id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?;
    if ticket_count > 0 {
        return Err(RepoError::Conflict(
            "cannot delete component with tickets".to_string(),
        ));
    }

    sqlx::query("DELETE FROM components WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::CreateUserRequest;
    use crate::repos::user;

    async fn test_pool() -> SqlitePool {
        let pool = db::init_memory_pool().await.unwrap();
        db::run_migrations(&pool).await.unwrap();
        pool
    }

    /// Components require an owner_id FK, so seed a user first.
    async fn seed_user(pool: &SqlitePool) -> i64 {
        let req = CreateUserRequest {
            login: "testowner".to_string(),
            display_name: "Test Owner".to_string(),
            email: "owner@test.com".to_string(),
            password: None,
            role: None,
        };
        user::create(pool, &req, None).await.unwrap().id
    }

    /// Helper that auto-generates a slug for root components (parent_id=None).
    fn make_create_request(name: &str, parent_id: Option<i64>, owner_id: i64) -> CreateComponentRequest {
        let slug = if parent_id.is_none() {
            Some(name.to_uppercase())
        } else {
            None
        };
        CreateComponentRequest {
            name: name.to_string(),
            parent_id,
            slug,
            owner_id,
        }
    }

    // ── slug validation (unit) ──────────────────────────────────────────

    #[test]
    fn validate_slug_valid() {
        assert!(validate_slug("AB").is_ok());
        assert!(validate_slug("PLAT").is_ok());
        assert!(validate_slug("NET42").is_ok());
        assert!(validate_slug("ABCDEFGHIJ").is_ok()); // 10 chars
    }

    #[test]
    fn validate_slug_too_short() {
        assert!(validate_slug("A").is_err());
    }

    #[test]
    fn validate_slug_too_long() {
        assert!(validate_slug("ABCDEFGHIJK").is_err()); // 11 chars
    }

    #[test]
    fn validate_slug_starts_with_digit() {
        assert!(validate_slug("1ABC").is_err());
    }

    #[test]
    fn validate_slug_lowercase() {
        assert!(validate_slug("plat").is_err());
        assert!(validate_slug("Plat").is_err());
    }

    // ── create with slug ────────────────────────────────────────────────

    #[tokio::test]
    async fn create_root_component() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let comp = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();

        assert_eq!(comp.name, "Platform");
        assert_eq!(comp.path, "/Platform/");
        assert_eq!(comp.slug.as_deref(), Some("PLATFORM"));
        assert!(comp.parent_id.is_none());
    }

    #[tokio::test]
    async fn create_root_without_slug_rejected() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let req = CreateComponentRequest {
            name: "NoSlug".to_string(),
            parent_id: None,
            slug: None,
            owner_id: owner,
        };
        let result = create(&pool, &req).await;
        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[tokio::test]
    async fn create_child_without_slug_ok() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let parent = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();
        let child = CreateComponentRequest {
            name: "Networking".to_string(),
            parent_id: Some(parent.id),
            slug: None,
            owner_id: owner,
        };
        let comp = create(&pool, &child).await.unwrap();
        assert!(comp.slug.is_none());
    }

    #[tokio::test]
    async fn create_with_invalid_slug_rejected() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        for bad in ["a", "ab", "1AB", "ABCDEFGHIJk"] {
            let req = CreateComponentRequest {
                name: "Test".to_string(),
                parent_id: None,
                slug: Some(bad.to_string()),
                owner_id: owner,
            };
            assert!(
                create(&pool, &req).await.is_err(),
                "expected error for slug '{bad}'"
            );
        }
    }

    #[tokio::test]
    async fn create_duplicate_slug_conflict() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        create(&pool, &CreateComponentRequest {
            name: "Alpha".to_string(),
            parent_id: None,
            slug: Some("PLAT".to_string()),
            owner_id: owner,
        })
        .await
        .unwrap();

        let result = create(&pool, &CreateComponentRequest {
            name: "Beta".to_string(),
            parent_id: None,
            slug: Some("PLAT".to_string()),
            owner_id: owner,
        })
        .await;
        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[tokio::test]
    async fn create_child_component() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let parent = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();
        let child = create(&pool, &make_create_request("Networking", Some(parent.id), owner))
            .await
            .unwrap();

        assert_eq!(child.path, "/Platform/Networking/");
        assert_eq!(child.parent_id, Some(parent.id));
    }

    #[tokio::test]
    async fn create_duplicate_name_conflict() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();

        let req = CreateComponentRequest {
            name: "Platform".to_string(),
            parent_id: None,
            slug: Some("PLAT2".to_string()),
            owner_id: owner,
        };
        let result = create(&pool, &req).await;
        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[tokio::test]
    async fn create_invalid_parent_not_found() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let result = create(&pool, &make_create_request("Orphan", Some(9999), owner)).await;
        assert!(matches!(result, Err(RepoError::NotFound)));
    }

    #[tokio::test]
    async fn get_by_id_found_and_not_found() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let comp = create(&pool, &make_create_request("Auth", None, owner))
            .await
            .unwrap();

        assert!(get_by_id(&pool, comp.id).await.unwrap().is_some());
        assert!(get_by_id(&pool, 9999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_ordered_by_path() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let parent = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();
        create(&pool, &make_create_request("Networking", Some(parent.id), owner))
            .await
            .unwrap();
        create(&pool, &make_create_request("Auth", None, owner))
            .await
            .unwrap();

        let all = list(&pool).await.unwrap();
        let paths: Vec<&str> = all.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(paths, vec!["/Auth/", "/Platform/", "/Platform/Networking/"]);
    }

    #[tokio::test]
    async fn test_get_children() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let platform = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();
        create(&pool, &make_create_request("Auth", None, owner))
            .await
            .unwrap();
        create(&pool, &make_create_request("Networking", Some(platform.id), owner))
            .await
            .unwrap();
        create(&pool, &make_create_request("Storage", Some(platform.id), owner))
            .await
            .unwrap();

        // Root children
        let roots = get_children(&pool, None).await.unwrap();
        assert_eq!(roots.len(), 2);

        // Platform's children
        let children = get_children(&pool, Some(platform.id)).await.unwrap();
        assert_eq!(children.len(), 2);
        let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Networking", "Storage"]);
    }

    #[tokio::test]
    async fn test_get_subtree() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let platform = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();
        let net = create(&pool, &make_create_request("Networking", Some(platform.id), owner))
            .await
            .unwrap();
        create(&pool, &make_create_request("DNS", Some(net.id), owner))
            .await
            .unwrap();
        create(&pool, &make_create_request("Auth", None, owner))
            .await
            .unwrap();

        let subtree = get_subtree(&pool, "/Platform/").await.unwrap();
        assert_eq!(subtree.len(), 3); // Platform, Networking, DNS

        let paths: Vec<&str> = subtree.iter().map(|c| c.path.as_str()).collect();
        assert!(paths.contains(&"/Platform/"));
        assert!(paths.contains(&"/Platform/Networking/"));
        assert!(paths.contains(&"/Platform/Networking/DNS/"));
        assert!(!paths.contains(&"/Auth/"));
    }

    // ── update with slug ────────────────────────────────────────────────

    #[tokio::test]
    async fn update_set_slug() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let parent = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();
        let child = create(&pool, &make_create_request("Net", Some(parent.id), owner))
            .await
            .unwrap();
        assert!(child.slug.is_none());

        let updated = update(
            &pool,
            child.id,
            &UpdateComponentRequest {
                name: None,
                parent_id: None,
                slug: Some(Some("NET".to_string())),
                owner_id: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(updated.slug.as_deref(), Some("NET"));
    }

    #[tokio::test]
    async fn update_clear_slug_on_child_ok() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let parent = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();
        let child = create(&pool, &CreateComponentRequest {
            name: "Net".to_string(),
            parent_id: Some(parent.id),
            slug: Some("NET".to_string()),
            owner_id: owner,
        })
        .await
        .unwrap();
        assert_eq!(child.slug.as_deref(), Some("NET"));

        let updated = update(
            &pool,
            child.id,
            &UpdateComponentRequest {
                name: None,
                parent_id: None,
                slug: Some(None),
                owner_id: None,
            },
        )
        .await
        .unwrap();
        assert!(updated.slug.is_none());
    }

    #[tokio::test]
    async fn update_clear_slug_on_root_rejected() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let root = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();

        let result = update(
            &pool,
            root.id,
            &UpdateComponentRequest {
                name: None,
                parent_id: None,
                slug: Some(None),
                owner_id: None,
            },
        )
        .await;
        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[tokio::test]
    async fn update_reparent_to_root_without_slug_rejected() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let parent = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();
        let child = create(&pool, &make_create_request("Net", Some(parent.id), owner))
            .await
            .unwrap();

        // Move child to root without giving it a slug
        let result = update(
            &pool,
            child.id,
            &UpdateComponentRequest {
                name: None,
                parent_id: Some(None),
                slug: None, // keep existing (None)
                owner_id: None,
            },
        )
        .await;
        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[tokio::test]
    async fn update_rename() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let parent = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();
        let child = create(&pool, &make_create_request("Net", Some(parent.id), owner))
            .await
            .unwrap();
        let grandchild = create(&pool, &make_create_request("DNS", Some(child.id), owner))
            .await
            .unwrap();

        let updated = update(
            &pool,
            child.id,
            &UpdateComponentRequest {
                name: Some("Networking".to_string()),
                parent_id: None,
                slug: None,
                owner_id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.path, "/Platform/Networking/");

        // Descendant path should be cascaded
        let gc = get_by_id(&pool, grandchild.id).await.unwrap().unwrap();
        assert_eq!(gc.path, "/Platform/Networking/DNS/");
    }

    #[tokio::test]
    async fn update_reparent() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let platform = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();
        let infra = create(&pool, &make_create_request("Infra", None, owner))
            .await
            .unwrap();
        let net = create(&pool, &make_create_request("Networking", Some(platform.id), owner))
            .await
            .unwrap();
        let dns = create(&pool, &make_create_request("DNS", Some(net.id), owner))
            .await
            .unwrap();

        // Move Networking under Infra
        let updated = update(
            &pool,
            net.id,
            &UpdateComponentRequest {
                name: None,
                parent_id: Some(Some(infra.id)),
                slug: None,
                owner_id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.path, "/Infra/Networking/");
        assert_eq!(updated.parent_id, Some(infra.id));

        let dns_updated = get_by_id(&pool, dns.id).await.unwrap().unwrap();
        assert_eq!(dns_updated.path, "/Infra/Networking/DNS/");
    }

    #[tokio::test]
    async fn update_reparent_to_root() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let platform = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();
        let net = create(&pool, &CreateComponentRequest {
            name: "Networking".to_string(),
            parent_id: Some(platform.id),
            slug: Some("NET".to_string()),
            owner_id: owner,
        })
        .await
        .unwrap();
        let dns = create(&pool, &make_create_request("DNS", Some(net.id), owner))
            .await
            .unwrap();

        // Move Networking to root — has slug so it's allowed
        let updated = update(
            &pool,
            net.id,
            &UpdateComponentRequest {
                name: None,
                parent_id: Some(None),
                slug: None,
                owner_id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.path, "/Networking/");
        assert!(updated.parent_id.is_none());

        let dns_updated = get_by_id(&pool, dns.id).await.unwrap().unwrap();
        assert_eq!(dns_updated.path, "/Networking/DNS/");
    }

    #[tokio::test]
    async fn update_circular_reference_rejected() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let platform = create(&pool, &make_create_request("Platform", None, owner))
            .await
            .unwrap();
        let net = create(&pool, &make_create_request("Networking", Some(platform.id), owner))
            .await
            .unwrap();
        let dns = create(&pool, &make_create_request("DNS", Some(net.id), owner))
            .await
            .unwrap();

        // Try to move Platform under DNS (its own grandchild)
        let result = update(
            &pool,
            platform.id,
            &UpdateComponentRequest {
                name: None,
                parent_id: Some(Some(dns.id)),
                slug: None,
                owner_id: None,
            },
        )
        .await;

        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[tokio::test]
    async fn delete_leaf_component() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let comp = create(&pool, &make_create_request("Leaf", None, owner))
            .await
            .unwrap();

        delete(&pool, comp.id).await.unwrap();
        assert!(get_by_id(&pool, comp.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_with_children_rejected() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let parent = create(&pool, &make_create_request("Parent", None, owner))
            .await
            .unwrap();
        create(&pool, &make_create_request("Child", Some(parent.id), owner))
            .await
            .unwrap();

        let result = delete(&pool, parent.id).await;
        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[tokio::test]
    async fn delete_with_tickets_rejected() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;
        let comp = create(&pool, &make_create_request("WithTicket", None, owner))
            .await
            .unwrap();

        // Insert a ticket via raw SQL to satisfy FK
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO tickets (type, title, owner_id, component_id, created_by, created_at, updated_at)
             VALUES ('bug', 'test ticket', ?, ?, ?, ?, ?)",
        )
        .bind(owner)
        .bind(comp.id)
        .bind(owner)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let result = delete(&pool, comp.id).await;
        assert!(matches!(result, Err(RepoError::Conflict(_))));
    }

    #[tokio::test]
    async fn count_returns_correct_total() {
        let pool = test_pool().await;
        let owner = seed_user(&pool).await;

        assert_eq!(count(&pool).await.unwrap(), 0);

        create(&pool, &make_create_request("Alpha", None, owner)).await.unwrap();
        create(&pool, &make_create_request("Beta", None, owner)).await.unwrap();

        assert_eq!(count(&pool).await.unwrap(), 2);
    }
}
