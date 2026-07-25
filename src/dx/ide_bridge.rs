use std::process::Command;

pub enum Ide {
    VSCode,
    Cursor,
    WebStorm,
}

impl Ide {
    fn protocol(&self) -> &'static str {
        match self {
            Ide::VSCode => "vscode://file",
            Ide::Cursor => "cursor://file",
            Ide::WebStorm => "webstorm://file",
        }
    }
}

pub struct IdeBridge;

impl IdeBridge {
    pub fn open_file(ide: Ide, file_path: &str, line: Option<usize>, col: Option<usize>) -> Result<(), String> {
        let mut uri = format!("{}{}", ide.protocol(), file_path);
        if let Some(l) = line {
            uri.push_str(&format!(":{}", l));
            if let Some(c) = col {
                uri.push_str(&format!(":{}", c));
            }
        }

        Self::launch_uri(&uri)
    }

    fn launch_uri(uri: &str) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            Command::new("open")
                .arg(uri)
                .status()
                .map_err(|e| e.to_string())?;
        }

        #[cfg(target_os = "windows")]
        {
            Command::new("cmd")
                .args(&["/c", "start", "", uri])
                .status()
                .map_err(|e| e.to_string())?;
        }

        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open")
                .arg(uri)
                .status()
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }
}
