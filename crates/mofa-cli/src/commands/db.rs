//! `mofa db` command implementation

use crate::CliError;
use crate::cli::DatabaseType;
use colored::Colorize;
use std::path::PathBuf;

/// Execute the `mofa db init` command
pub fn run_init(
    db_type: DatabaseType,
    output: Option<PathBuf>,
    database_url: Option<String>,
) -> Result<(), CliError> {
    let sql = get_migration_sql(db_type);

    if let Some(url) = database_url {
        #[cfg(feature = "db")]
        {
            println!(
                "{} Initializing {} database...",
                "→".green(),
                db_type.to_string().cyan()
            );
            println!("  URL: {}", mask_password(&url));

            // Execute SQL against database
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async { execute_migration(db_type, &url, sql).await })?;

            println!("{} Database tables initialized successfully!", "✓".green());
        }

        #[cfg(not(feature = "db"))]
        {
            let _ = url; // suppress unused variable warning
            return Err(CliError::Other(format!(
                "Direct database execution requires the 'db' feature.\n\
                 Build with: cargo install mofa-cli --features db\n\
                 Or output to file: mofa db init -t {} -o migration.sql",
                db_type
            )));
        }
    } else if let Some(output_path) = output {
        // Write SQL to file
        println!(
            "{} Generating {} migration script...",
            "→".green(),
            db_type.to_string().cyan()
        );
        std::fs::write(&output_path, sql)?;
        println!(
            "{} Migration script saved to: {}",
            "✓".green(),
            output_path.display()
        );
    } else {
        // Print SQL to stdout
        println!("{}", sql);
    }

    Ok(())
}

/// Execute the `mofa db schema` command
pub fn run_schema(db_type: DatabaseType) -> Result<(), CliError> {
    let sql = get_migration_sql(db_type);
    println!("-- MoFA {} Schema", db_type.to_string().to_uppercase());
    println!("-- Copy and execute this SQL to initialize your database\n");
    println!("{}", sql);
    Ok(())
}

fn get_migration_sql(db_type: DatabaseType) -> &'static str {
    match db_type {
        DatabaseType::Postgres => {
            include_str!("../../../../scripts/sql/migrations/postgres_init.sql")
        }
        DatabaseType::Mysql => include_str!("../../../../scripts/sql/migrations/mysql_init.sql"),
        DatabaseType::Sqlite => include_str!("../../../../scripts/sql/migrations/sqlite_init.sql"),
    }
}

#[cfg(feature = "db")]
fn mask_password(url: &str) -> String {
    // Mask password in database URL for display
    if let Some(at_pos) = url.find('@')
        && let Some(colon_pos) = url[..at_pos].rfind(':')
    {
        let prefix = &url[..colon_pos + 1];
        let suffix = &url[at_pos..];
        return format!("{}****{}", prefix, suffix);
    }
    url.to_string()
}

#[cfg(feature = "db")]
async fn execute_migration(db_type: DatabaseType, url: &str, sql: &str) -> Result<(), CliError> {
    match db_type {
        DatabaseType::Postgres => {
            use sqlx::Executor;
            use sqlx::postgres::PgPoolOptions;

            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(url)
                .await
                .map_err(|e| CliError::Other(format!("Failed to connect to PostgreSQL: {}", e)))?;

            // Execute each statement separately for PostgreSQL
            for statement in sql.split(';') {
                let stmt = statement.trim();
                if !stmt.is_empty() && !stmt.starts_with("--") {
                    pool.execute(stmt)
                        .await
                        .map_err(|e| CliError::Other(format!("SQL error: {}", e)))?;
                }
            }

            pool.close().await;
        }
        DatabaseType::Mysql => {
            use sqlx::Executor;
            use sqlx::mysql::MySqlPoolOptions;

            let pool = MySqlPoolOptions::new()
                .max_connections(1)
                .connect(url)
                .await
                .map_err(|e| CliError::Other(format!("Failed to connect to MySQL: {}", e)))?;

            // Execute each statement separately for MySQL
            for statement in sql.split(';') {
                let stmt = statement.trim();
                if !stmt.is_empty() && !stmt.starts_with("--") && !stmt.starts_with("SELECT") {
                    pool.execute(stmt)
                        .await
                        .map_err(|e| CliError::Other(format!("SQL error: {}", e)))?;
                }
            }

            pool.close().await;
        }
        DatabaseType::Sqlite => {
            use sqlx::Executor;
            use sqlx::sqlite::SqlitePoolOptions;

            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect(url)
                .await
                .map_err(|e| CliError::Other(format!("Failed to connect to SQLite: {}", e)))?;

            // Execute each statement separately for SQLite
            for statement in sql.split(';') {
                let stmt = statement.trim();
                if !stmt.is_empty() && !stmt.starts_with("--") {
                    pool.execute(stmt)
                        .await
                        .map_err(|e| CliError::Other(format!("SQL error: {}", e)))?;
                }
            }

            pool.close().await;
        }
    }

    Ok(())
}
