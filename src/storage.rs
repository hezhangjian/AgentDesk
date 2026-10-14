use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use anyhow::{Context, Result};

use crate::model::Workspace;

pub struct Storage {
    pub directory: PathBuf,
}

impl Storage {
    pub fn discover() -> Result<Self> {
        let directory = if let Some(path) = std::env::var_os("AGENTDESK_DATA_DIR") {
            PathBuf::from(path)
        } else {
            dirs::data_local_dir()
                .context("无法定位应用数据目录")?
                .join("AgentDesk")
        };
        Ok(Self { directory })
    }

    pub fn load(&self) -> Result<Workspace> {
        let path = self.directory.join("workspace.json");
        let data = match fs::read(&path) {
            Ok(data) => data,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Workspace::default());
            }
            Err(error) => {
                return Err(error).with_context(|| format!("无法读取 {}", path.display()));
            }
        };
        let mut workspace: Workspace = serde_json::from_slice(&data)
            .with_context(|| format!("数据文件损坏，原文件已保留：{}", path.display()))?;
        anyhow::ensure!(
            workspace.version == 1,
            "不支持的数据版本 {}，原文件已保留",
            workspace.version
        );
        workspace.recover();
        Ok(workspace)
    }

    pub fn save(&self, workspace: &Workspace) -> Result<()> {
        fs::create_dir_all(&self.directory)?;
        let path = self.directory.join("workspace.json");
        let temporary = self.directory.join("workspace.json.tmp");
        let data = serde_json::to_vec_pretty(workspace)?;
        let mut options = OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&data)?;
        file.sync_all()?;
        fs::rename(temporary, path)?;
        Ok(())
    }
}
