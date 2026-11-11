//! Base SQLite : profils de voix, echantillons, historique des generations.
use crate::{e, R};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::Path;

pub struct Store(Connection);

pub struct ProfileRef {
    pub id: i64,
    pub name: String,
    pub language: String,
}

pub struct Generation {
    pub text: String,
    pub original: String,
    pub enhanced: Option<String>,
}

impl Generation {
    /// Fichier de la version en vigueur : amelioree si activee, sinon originale.
    pub fn active(&self) -> &str {
        self.enhanced.as_deref().unwrap_or(&self.original)
    }
}

pub struct Sample {
    pub id: i64,
    pub audio: String,
    pub ref_text: String,
}

impl Store {
    pub fn open(path: &Path) -> R<Self> {
        let c = Connection::open(path).map_err(e)?;
        c.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS profiles(id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE,
               language TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));
             CREATE TABLE IF NOT EXISTS samples(id INTEGER PRIMARY KEY,
               profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
               audio_path TEXT NOT NULL, ref_text TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS generations(id INTEGER PRIMARY KEY,
               profile_id INTEGER REFERENCES profiles(id) ON DELETE SET NULL,
               text TEXT NOT NULL, audio_path TEXT NOT NULL, duration REAL NOT NULL, seed INTEGER NOT NULL,
               created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));
             CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .map_err(e)?;
        // Migration : version amelioree d'une generation (NULL = original seul).
        let has = c
            .prepare("SELECT 1 FROM pragma_table_info('generations') WHERE name='enhanced_path'")
            .and_then(|mut s| s.exists([]))
            .map_err(e)?;
        if !has {
            c.execute("ALTER TABLE generations ADD COLUMN enhanced_path TEXT", []).map_err(e)?;
        }
        Ok(Store(c))
    }

    pub fn setting(&self, key: &str) -> Option<String> {
        self.0.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get(0)).ok()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> R<()> {
        self.0
            .execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=?2", params![key, value])
            .map(|_| ())
            .map_err(e)
    }

    // Execute une ecriture qui doit toucher exactement une ligne.
    fn one(&self, sql: &str, p: impl rusqlite::Params, what: &str) -> R<()> {
        match self.0.execute(sql, p).map_err(e)? {
            1 => Ok(()),
            _ => Err(format!("{what} introuvable")),
        }
    }

    // Chemin d'un fichier audio enregistre dans `table`.
    fn path(&self, table: &str, id: i64) -> R<String> {
        self.0
            .query_row(&format!("SELECT audio_path FROM {table} WHERE id=?1"), [id], |r| r.get(0))
            .map_err(|_| format!("élément {id} introuvable"))
    }

    pub fn add_profile(&self, name: &str, language: &str) -> R<i64> {
        self.0
            .execute("INSERT INTO profiles(name,language) VALUES(?1,?2)", params![name, language])
            .map_err(|x| format!("profil « {name} » : {x}"))?;
        Ok(self.0.last_insert_rowid())
    }

    pub fn delete_profile(&self, id: i64) -> R<()> {
        self.one("DELETE FROM profiles WHERE id=?1", [id], "profil")
    }

    /// Profil par nom, ou le premier si `None`.
    pub fn find_profile(&self, name: Option<&str>) -> R<ProfileRef> {
        self.0
            .query_row(
                "SELECT id,name,language FROM profiles WHERE ?1 IS NULL OR name=?1 ORDER BY id LIMIT 1",
                params![name],
                |r| Ok(ProfileRef { id: r.get(0)?, name: r.get(1)?, language: r.get(2)? }),
            )
            .map_err(|_| match name {
                Some(n) => format!("profil introuvable : {n}"),
                None => "aucun profil de voix".to_string(),
            })
    }

    pub fn list_profiles(&self) -> R<Value> {
        let mut st = self.0
            .prepare("SELECT p.id,p.name,p.language,(SELECT COUNT(*) FROM samples s WHERE s.profile_id=p.id) FROM profiles p ORDER BY p.id")
            .map_err(e)?;
        let rows = st
            .query_map([], |r| {
                Ok(json!({"id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "language": r.get::<_, String>(2)?, "samples": r.get::<_, i64>(3)?}))
            })
            .map_err(e)?;
        Ok(Value::Array(rows.collect::<Result<_, _>>().map_err(e)?))
    }

    pub fn add_sample(&self, profile_id: i64, audio_path: &str, ref_text: &str) -> R<i64> {
        self.0
            .execute("INSERT INTO samples(profile_id,audio_path,ref_text) VALUES(?1,?2,?3)", params![profile_id, audio_path, ref_text])
            .map_err(|_| format!("profil {profile_id} introuvable"))?;
        Ok(self.0.last_insert_rowid())
    }

    pub fn samples(&self, profile_id: i64) -> R<Vec<Sample>> {
        let mut st = self.0.prepare("SELECT id,audio_path,ref_text FROM samples WHERE profile_id=?1 ORDER BY id").map_err(e)?;
        let rows = st.query_map([profile_id], |r| Ok(Sample { id: r.get(0)?, audio: r.get(1)?, ref_text: r.get(2)? })).map_err(e)?;
        rows.collect::<Result<_, _>>().map_err(e)
    }

    pub fn set_sample_text(&self, id: i64, text: &str) -> R<()> {
        self.one("UPDATE samples SET ref_text=?2 WHERE id=?1", params![id, text], "échantillon")
    }

    pub fn sample_path(&self, id: i64) -> R<String> {
        self.path("samples", id)
    }

    pub fn delete_sample(&self, id: i64) -> R<()> {
        self.one("DELETE FROM samples WHERE id=?1", [id], "échantillon")
    }

    pub fn add_generation(&self, profile_id: i64, text: &str, audio_path: &str, duration: f64, seed: i64) -> R<i64> {
        self.0
            .execute(
                "INSERT INTO generations(profile_id,text,audio_path,duration,seed) VALUES(?1,?2,?3,?4,?5)",
                params![profile_id, text, audio_path, duration, seed],
            )
            .map_err(e)?;
        Ok(self.0.last_insert_rowid())
    }

    pub fn generation(&self, id: i64) -> R<Generation> {
        self.0
            .query_row("SELECT text,audio_path,enhanced_path FROM generations WHERE id=?1", [id], |r| {
                Ok(Generation { text: r.get(0)?, original: r.get(1)?, enhanced: r.get(2)? })
            })
            .map_err(|_| format!("génération {id} introuvable"))
    }

    pub fn set_enhanced_path(&self, id: i64, path: Option<&str>) -> R<()> {
        self.one("UPDATE generations SET enhanced_path=?2 WHERE id=?1", params![id, path], "génération")
    }

    pub fn delete_generation(&self, id: i64) -> R<()> {
        self.one("DELETE FROM generations WHERE id=?1", [id], "génération")
    }

    pub fn list_generations(&self, limit: i64) -> R<Value> {
        let mut st = self.0
            .prepare(
                "SELECT g.id,p.name,g.text,COALESCE(g.enhanced_path,g.audio_path),g.duration,g.seed,g.created_at,g.enhanced_path IS NOT NULL FROM generations g
                 LEFT JOIN profiles p ON p.id=g.profile_id ORDER BY g.id DESC LIMIT ?1",
            )
            .map_err(e)?;
        let rows = st
            .query_map(params![limit.clamp(1, 500)], |r| {
                Ok(json!({"id": r.get::<_, i64>(0)?, "profile": r.get::<_, Option<String>>(1)?, "text": r.get::<_, String>(2)?,
                    "path": r.get::<_, String>(3)?, "duration_sec": r.get::<_, f64>(4)?, "seed": r.get::<_, i64>(5)?, "created_at": r.get::<_, String>(6)?,
                    "enhanced": r.get::<_, bool>(7)?}))
            })
            .map_err(e)?;
        Ok(Value::Array(rows.collect::<Result<_, _>>().map_err(e)?))
    }
}
