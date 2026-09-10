// SPDX-License-Identifier: GPL-3.0-only

use cosmic::Application;
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

use crate::App;

const SESSION_VERSION: u16 = 1;
const SESSION_FILE: &str = "session.json";
const TABS_DIR: &str = "tabs";

/// Persisted open tabs
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Session {
    pub version: u16,
    pub active: usize,
    pub tabs: Vec<SessionTab>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionTab {
    pub path: Option<PathBuf>,
    pub text_file: Option<String>,
    pub cursor: (usize, usize),
    pub scroll: (usize, f32, f32),
    #[serde(skip)]
    pub text: Option<String>,
}

pub fn session_dir() -> Option<PathBuf> {
    dirs::state_dir()
        .or_else(dirs::config_dir)
        .map(|dir| dir.join(App::APP_ID).join("session"))
}

impl Session {
    pub fn load() -> Option<Session> {
        let dir = session_dir()?;
        let mut session: Session =
            match serde_json::from_str(&fs::read_to_string(dir.join(SESSION_FILE)).ok()?) {
                Ok(ok) => ok,
                Err(err) => {
                    log::warn!("failed to parse session file: {}", err);
                    return None;
                }
            };
        if session.version != SESSION_VERSION {
            log::warn!(
                "ignoring session file with unsupported version {}",
                session.version
            );
            return None;
        }

        for tab in session.tabs.iter_mut() {
            if let Some(text_file) = &tab.text_file {
                tab.text = match fs::read_to_string(dir.join(text_file)) {
                    Ok(ok) => Some(ok),
                    Err(err) => {
                        log::warn!(
                            "failed to read unsaved tab contents {:?}: {}",
                            text_file,
                            err
                        );
                        None
                    }
                };
            }
        }
        Some(session)
    }

    pub fn save(&self) -> io::Result<()> {
        let Some(dir) = session_dir() else {
            log::warn!("no directory available to save the session");
            return Ok(());
        };

        let tabs_dir = dir.join(TABS_DIR);
        if tabs_dir.exists() {
            fs::remove_dir_all(&tabs_dir)?;
        }
        fs::create_dir_all(&tabs_dir)?;

        for tab in self.tabs.iter() {
            if let (Some(text_file), Some(text)) = (&tab.text_file, &tab.text) {
                let path = dir.join(text_file);
                let mut tmp = path.clone().into_os_string();
                tmp.push(".tmp");
                let tmp = PathBuf::from(tmp);
                fs::write(&tmp, text)?;
                fs::rename(&tmp, &path)?;
            }
        }

        let path = dir.join(SESSION_FILE);
        let mut tmp = path.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn text_file_name(position: usize) -> String {
        format!("{}/{}.txt", TABS_DIR, position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_session() -> Session {
        Session {
            version: SESSION_VERSION,
            active: 1,
            tabs: vec![
                SessionTab {
                    path: Some(PathBuf::from("/tmp/a.txt")),
                    text_file: None,
                    cursor: (3, 5),
                    scroll: (2, 120.5, 0.0),
                    text: None,
                },
                SessionTab {
                    path: None,
                    text_file: Some(Session::text_file_name(1)),
                    cursor: (0, 0),
                    scroll: (0, 0.0, 0.0),
                    text: Some("unsaved\n".to_string()),
                },
                SessionTab {
                    path: Some(PathBuf::from("/tmp/b.txt")),
                    text_file: Some(Session::text_file_name(2)),
                    cursor: (1, 1),
                    scroll: (1, 42.0, 0.0),
                    text: Some("dirty\n".to_string()),
                },
            ],
        }
    }

    fn with_temp_session_dir(f: impl FnOnce()) {
        use std::sync::{Mutex, MutexGuard, OnceLock};

        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard: MutexGuard<'_, ()> = LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|err| err.into_inner());

        let tmp = std::env::temp_dir().join(format!("cosmic-edit-test-{}", std::process::id()));
        let state = tmp.join("state");
        let old = std::env::var_os("XDG_STATE_HOME");
        std::fs::create_dir_all(&state).unwrap();
        unsafe {
            std::env::set_var("XDG_STATE_HOME", &state);
        }
        f();
        unsafe {
            match old {
                Some(value) => std::env::set_var("XDG_STATE_HOME", value),
                None => std::env::remove_var("XDG_STATE_HOME"),
            }
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn session_roundtrip() {
        with_temp_session_dir(|| {
            let session = test_session();
            session.save().unwrap();

            let loaded = Session::load().expect("session should load");
            assert_eq!(loaded.active, 1);
            assert_eq!(loaded.tabs.len(), 3);
            assert_eq!(loaded.tabs[0].path, Some(PathBuf::from("/tmp/a.txt")));
            assert!(loaded.tabs[0].text.is_none());
            assert_eq!(loaded.tabs[1].text.as_deref(), Some("unsaved\n"));
            assert_eq!(loaded.tabs[2].text.as_deref(), Some("dirty\n"));
            assert_eq!(loaded.tabs[2].cursor, (1, 1));
        });
    }

    #[test]
    fn session_json_hides_unsaved_text() {
        with_temp_session_dir(|| {
            let session = test_session();
            session.save().unwrap();

            let dir = session_dir().unwrap();
            let json = fs::read_to_string(dir.join(SESSION_FILE)).unwrap();
            assert!(
                !json.contains("unsaved\\n"),
                "raw text must not be inlined: {json}"
            );
            assert!(dir.join(TABS_DIR).join("1.txt").exists());
        });
    }

    #[test]
    fn save_clears_stale_tab_files() {
        with_temp_session_dir(|| {
            test_session().save().unwrap();
            let dir = session_dir().unwrap().join(TABS_DIR);
            assert!(dir.join("1.txt").exists());
            assert!(dir.join("2.txt").exists());

            let mut smaller = test_session();
            smaller.tabs.truncate(1);
            smaller.save().unwrap();
            assert!(!dir.join("1.txt").exists());
            assert!(!dir.join("2.txt").exists());
        });
    }
}
