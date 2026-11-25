use std::path::Path;

use anyhow::{Result, bail};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use serde_json::{Value, json};

use crate::project::Project;

/// Inspect SQLite metadata only. No user SQL is accepted; the connection is
/// opened read-only and cannot create a missing database or run migrations.
pub async fn inspect(project: &Project, database: &Path) -> Result<Value> {
    if database.is_absolute() {
        bail!("database must be a project-relative file path");
    }
    let path = project.existing_file(database)?;
    let url = url::Url::from_file_path(&path).map_err(|()| anyhow::anyhow!("invalid database path"))?;
    let connection = Database::connect(format!("sqlite://{}?mode=ro", url.path())).await?;
    let rows = connection.query_all(Statement::from_string(DatabaseBackend::Sqlite,
        "SELECT name, type FROM sqlite_schema WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' ORDER BY name LIMIT 201".to_owned()
    )).await?;
    let truncated = rows.len() > 200;
    let mut tables = Vec::new();
    for row in rows.into_iter().take(200) {
        let name: String = row.try_get("", "name")?;
        let kind: String = row.try_get("", "type")?;
        let columns = connection
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT name, type, \"notnull\" AS required, pk FROM pragma_table_info(?) LIMIT 201",
                [name.clone().into()],
            ))
            .await?;
        let columns_truncated = columns.len() > 200;
        let columns = columns
            .into_iter()
            .take(200)
            .map(|column| -> Result<Value> {
                Ok(json!({
                    "name": column.try_get::<String>("", "name")?,
                    "type": column.try_get::<String>("", "type")?,
                    "required": column.try_get::<i32>("", "required")? != 0,
                    "primary_key_position": column.try_get::<i32>("", "pk")?,
                }))
            })
            .collect::<Result<Vec<_>>>()?;
        tables.push(json!({"name": name, "type": kind, "columns": columns, "columns_truncated": columns_truncated}));
    }
    connection.close().await?;
    Ok(json!({"tables": tables, "truncated": truncated}))
}
