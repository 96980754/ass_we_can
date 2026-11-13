use std::fs;
use std::io::Write;
use std::path::Path;

use crate::model::{self, Rep, Settings};

pub struct Store {
    pub settings: Settings,
    pub reps: Vec<Rep>,
    pub warning: Option<String>,
}

impl Store {
    pub fn load(dir: &Path) -> Store {
        let _ = fs::create_dir_all(dir);
        let mut warning = None;

        let settings_path = dir.join("settings.json");
        let mut settings = Settings::default();

        if let Ok(text) = fs::read_to_string(&settings_path) {
            match serde_json::from_str::<Settings>(&text) {
                Ok(parsed) => settings = parsed,
                Err(_) => {
                    let backup = dir.join("settings.json.bak");
                    let recovered = fs::read_to_string(&backup)
                        .ok()
                        .and_then(|t| serde_json::from_str::<Settings>(&t).ok());
                    match recovered {
                        Some(parsed) => {
                            settings = parsed;
                            warning = Some("设置文件损坏，已从备份恢复".to_string());
                        }
                        None => {
                            let corrupt =
                                dir.join(format!("settings.json.corrupt-{}", model::now_ms()));
                            let _ = fs::rename(&settings_path, corrupt);
                            warning = Some("设置文件损坏，已重置为默认值".to_string());
                        }
                    }
                }
            }
        }

        let mut reps = Vec::new();
        if let Ok(text) = fs::read_to_string(dir.join("reps.jsonl")) {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Ok(rep) = serde_json::from_str::<Rep>(line) {
                    reps.push(rep);
                }
            }
        }

        Store {
            settings,
            reps,
            warning,
        }
    }

    /// 临时文件 + fsync + rename，避免写一半掉电留下坏文件；同时保留上一版备份。
    pub fn save_settings(&self, dir: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        let path = dir.join("settings.json");
        let tmp = dir.join("settings.json.tmp");
        let data = serde_json::to_vec_pretty(&self.settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(&data)?;
            file.sync_all()?;
        }
        if path.exists() {
            let _ = fs::copy(&path, dir.join("settings.json.bak"));
        }
        fs::rename(&tmp, &path)
    }

    pub fn append_rep(&self, dir: &Path, rep: &Rep) -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        let line = serde_json::to_string(rep)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("reps.jsonl"))?;
        writeln!(file, "{line}")?;
        file.flush()?;
        let _ = file.sync_data();
        Ok(())
    }

    pub fn clear_reps_file(&self, dir: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        fs::File::create(dir.join("reps.jsonl"))?.sync_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tigang-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn settings_roundtrip() {
        let dir = temp_dir();
        let store = Store {
            settings: Settings::default(),
            reps: vec![],
            warning: None,
        };
        store.save_settings(&dir).unwrap();
        let loaded = Store::load(&dir);
        assert_eq!(loaded.settings.interval_minutes, 45);
        assert_eq!(loaded.settings.target_seconds, 5);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn recovers_from_corrupt_settings() {
        let dir = temp_dir();
        fs::write(dir.join("settings.json"), "{ not json").unwrap();
        let loaded = Store::load(&dir);
        assert_eq!(loaded.settings.interval_minutes, 45);
        assert!(loaded.warning.is_some());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn append_and_reload_reps() {
        let dir = temp_dir();
        let store = Store {
            settings: Settings::default(),
            reps: vec![],
            warning: None,
        };
        let rep = Rep {
            id: "x".into(),
            started_at_ms: 1,
            duration_ms: 2000,
            local_date: "2026-11-13".into(),
            tz_offset_min: 480,
        };
        store.append_rep(&dir, &rep).unwrap();
        let loaded = Store::load(&dir);
        assert_eq!(loaded.reps.len(), 1);
        assert_eq!(loaded.reps[0].duration_ms, 2000);
        fs::remove_dir_all(dir).ok();
    }
}
