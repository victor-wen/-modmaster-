use hc_core::model::{Device, Project, Tag};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("TOML ser: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("Invalid project: {0}")]
    InvalidProject(String),
}

pub const PROJECT_FILE: &str = "project.toml";
pub const DEVICES_FILE: &str = "devices.toml";
pub const TAGS_FILE: &str = "tags.toml";

pub fn project_dir_name(name: &str) -> String {
    format!("{}.hcproj", name)
}
pub fn history_db_path(project_dir: &Path) -> PathBuf {
    project_dir.join("history.db")
}

pub fn create_project(path: &Path, project: &Project) -> Result<(), ProjectError> {
    fs::create_dir_all(path)?;
    fs::create_dir_all(path.join("dashboards"))?;
    save_project_file(path, project)?;
    save_devices(path, &[])?;
    save_tags(path, &[])?;
    Ok(())
}

pub fn load_project(path: &Path) -> Result<Project, ProjectError> {
    let content = fs::read_to_string(path.join(PROJECT_FILE))?;
    let mut project: Project = toml::from_str(&content)?;
    project.path = Some(path.to_path_buf());
    Ok(project)
}

pub fn save_project_file(path: &Path, project: &Project) -> Result<(), ProjectError> {
    fs::write(path.join(PROJECT_FILE), toml::to_string_pretty(project)?)?;
    Ok(())
}

pub fn load_devices(path: &Path) -> Result<Vec<Device>, ProjectError> {
    let p = path.join(DEVICES_FILE);
    if !p.exists() {
        return Ok(Vec::new());
    }
    #[derive(serde::Deserialize)]
    struct W {
        device: Vec<Device>,
    }
    Ok(toml::from_str::<W>(&fs::read_to_string(p)?)?.device)
}

pub fn save_devices(path: &Path, devices: &[Device]) -> Result<(), ProjectError> {
    #[derive(serde::Serialize)]
    struct W<'a> {
        device: &'a [Device],
    }
    fs::write(
        path.join(DEVICES_FILE),
        toml::to_string_pretty(&W { device: devices })?,
    )?;
    Ok(())
}

pub fn load_tags(path: &Path) -> Result<Vec<Tag>, ProjectError> {
    let p = path.join(TAGS_FILE);
    if !p.exists() {
        return Ok(Vec::new());
    }
    #[derive(serde::Deserialize)]
    struct W {
        tag: Vec<Tag>,
    }
    Ok(toml::from_str::<W>(&fs::read_to_string(p)?)?.tag)
}

pub fn save_tags(path: &Path, tags: &[Tag]) -> Result<(), ProjectError> {
    #[derive(serde::Serialize)]
    struct W<'a> {
        tag: &'a [Tag],
    }
    fs::write(
        path.join(TAGS_FILE),
        toml::to_string_pretty(&W { tag: tags })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hc_core::model::*;

    #[test]
    fn test_roundtrip() {
        let dir = std::env::temp_dir().join("hc_test_proj");
        let _ = fs::remove_dir_all(&dir);
        let p = Project::default();
        create_project(&dir, &p).unwrap();
        let loaded = load_project(&dir).unwrap();
        assert_eq!(loaded.name, p.name);
        let devs = vec![Device {
            id: "dev_1".into(),
            name: "Test".into(),
            enabled: true,
            protocol: "modbus".into(),
            transport: TransportSpec::Tcp {
                host: "127.0.0.1".into(),
                port: 502,
            },
            protocol_params: serde_json::json!({"slave_id": 1}),
            poll_interval_ms: 1000,
            timeout_ms: 1000,
        }];
        save_devices(&dir, &devs).unwrap();
        assert_eq!(load_devices(&dir).unwrap().len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_empty_devices_ok() {
        let dir = std::env::temp_dir().join("hc_test_empty");
        let _ = fs::create_dir_all(&dir);
        assert!(load_devices(&dir).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_history_db_path() {
        assert_eq!(history_db_path(Path::new("/p")), Path::new("/p/history.db"));
    }
}
