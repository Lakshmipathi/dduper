use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

pub struct CsumDb {
    conn: Connection,
}

impl CsumDb {
    /// Open or create the dduper.db SQLite database
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).context("Failed to open dduper.db")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS filehash (
                filename TEXT,
                short_hash TEXT,
                processed INTEGER DEFAULT 0,
                valid INTEGER DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS btrfscsum (
                short_hash TEXT,
                long_hash TEXT
            );",
        )
        .context("Failed to create tables")?;
        Ok(CsumDb { conn })
    }

    /// Insert checksum data for a file
    pub fn insert_csum(&self, filename: &str, short_hash: &str, csum_data: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO filehash VALUES (?1, ?2, 0, 0)",
            rusqlite::params![filename, short_hash],
        )?;

        // Only insert into btrfscsum if this hash doesn't exist yet
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM btrfscsum WHERE short_hash = ?1)",
            rusqlite::params![short_hash],
            |row| row.get(0),
        )?;

        if !exists {
            self.conn.execute(
                "INSERT INTO btrfscsum VALUES (?1, ?2)",
                rusqlite::params![short_hash, csum_data],
            )?;
        }

        Ok(())
    }

    /// Check if checksums are cached for a file, return them if so
    pub fn get_cached_csum(&self, filename: &str) -> Result<Option<String>> {
        let result: Option<String> = self
            .conn
            .query_row(
                "SELECT short_hash FROM filehash WHERE filename = ?1",
                rusqlite::params![filename],
                |row| row.get(0),
            )
            .optional()?;

        let short_hash = match result {
            Some(h) => h,
            None => return Ok(None),
        };

        let csum_data: Option<String> = self
            .conn
            .query_row(
                "SELECT long_hash FROM btrfscsum WHERE short_hash = ?1",
                rusqlite::params![short_hash],
                |row| row.get(0),
            )
            .optional()?;

        Ok(csum_data)
    }

    /// Mark a file as processed
    pub fn mark_processed(&self, filename: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE filehash SET processed = 1 WHERE filename = ?1",
            rusqlite::params![filename],
        )?;
        Ok(())
    }

    /// Mark a file as valid
    pub fn mark_valid(&self, filename: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE filehash SET valid = 1 WHERE filename = ?1",
            rusqlite::params![filename],
        )?;
        Ok(())
    }

    /// Detect groups of duplicate files (files with same short_hash)
    pub fn detect_duplicates(&self) -> Result<Vec<Vec<String>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT short_hash FROM filehash GROUP BY short_hash HAVING COUNT(*) > 1")?;

        let hashes: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let mut groups = Vec::new();
        let mut file_stmt = self
            .conn
            .prepare("SELECT filename FROM filehash WHERE short_hash = ?1")?;

        for hash in &hashes {
            let files: Vec<String> = file_stmt
                .query_map(rusqlite::params![hash], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            if files.len() > 1 {
                groups.push(files);
            }
        }

        Ok(groups)
    }

    /// Get files that are valid but not yet processed
    pub fn get_unprocessed(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT filename FROM filehash WHERE valid = 1 AND processed = 0")?;

        let files: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(files)
    }
}

/// Extension trait for optional query results
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for std::result::Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> CsumDb {
        // Use a file-like path that triggers in-memory behavior
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS filehash (
                filename TEXT, short_hash TEXT, processed INTEGER DEFAULT 0, valid INTEGER DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS btrfscsum (
                short_hash TEXT, long_hash TEXT
            );",
        )
        .unwrap();
        CsumDb { conn }
    }

    #[test]
    fn test_insert_and_retrieve() {
        let db = memory_db();
        db.insert_csum("/mnt/test.txt", "abc123", "0x1234 0x5678")
            .unwrap();
        let result = db.get_cached_csum("/mnt/test.txt").unwrap();
        assert_eq!(result, Some("0x1234 0x5678".to_string()));
    }

    #[test]
    fn test_cache_miss() {
        let db = memory_db();
        let result = db.get_cached_csum("/mnt/nonexistent.txt").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_mark_processed() {
        let db = memory_db();
        db.insert_csum("/mnt/f1", "hash1", "data1").unwrap();
        db.mark_valid("/mnt/f1").unwrap();
        db.mark_processed("/mnt/f1").unwrap();

        let unprocessed = db.get_unprocessed().unwrap();
        assert!(unprocessed.is_empty());
    }

    #[test]
    fn test_detect_duplicates() {
        let db = memory_db();
        db.insert_csum("/mnt/f1", "same_hash", "data").unwrap();
        db.insert_csum("/mnt/f2", "same_hash", "data").unwrap();
        db.insert_csum("/mnt/f3", "different", "other").unwrap();

        let groups = db.detect_duplicates().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 2);
    }

    #[test]
    fn test_get_unprocessed() {
        let db = memory_db();
        db.insert_csum("/mnt/f1", "h1", "d1").unwrap();
        db.insert_csum("/mnt/f2", "h2", "d2").unwrap();
        db.mark_valid("/mnt/f1").unwrap();
        db.mark_valid("/mnt/f2").unwrap();
        db.mark_processed("/mnt/f1").unwrap();

        let unprocessed = db.get_unprocessed().unwrap();
        assert_eq!(unprocessed, vec!["/mnt/f2".to_string()]);
    }

    #[test]
    fn test_duplicate_hash_not_reinserted() {
        let db = memory_db();
        db.insert_csum("/mnt/f1", "same_hash", "data1").unwrap();
        db.insert_csum("/mnt/f2", "same_hash", "data2").unwrap();

        // btrfscsum should have only one entry for this hash
        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM btrfscsum WHERE short_hash = 'same_hash'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
