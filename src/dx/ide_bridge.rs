use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            Ide::WebStorm => "webstorm://open",
        }
    }
}

pub struct IdeBridge;

impl IdeBridge {
    pub fn build_uri(ide: Ide, file_path: &str, line: Option<usize>, col: Option<usize>) -> String {
        let abs_path = std::fs::canonicalize(file_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file_path.to_string());

        match ide {
            Ide::WebStorm => {
                let mut u = format!("webstorm://open?file={}", abs_path);
                if let Some(l) = line {
                    u.push_str(&format!("&line={}", l));
                }
                if let Some(c) = col {
                    u.push_str(&format!("&column={}", c));
                }
                u
            }
            Ide::VSCode | Ide::Cursor => {
                let prefix = if abs_path.starts_with('/') { "" } else { "/" };
                let mut u = format!("{}{}{}", ide.protocol(), prefix, abs_path);
                if let Some(l) = line {
                    u.push_str(&format!(":{}", l));
                    if let Some(c) = col {
                        u.push_str(&format!(":{}", c));
                    }
                }
                u
            }
        }
    }

    pub fn open_file(ide: Ide, file_path: &str, line: Option<usize>, col: Option<usize>) -> Result<(), String> {
        let uri = Self::build_uri(ide, file_path, line, col);
        Self::launch_uri(&uri)
    }

    fn launch_uri(uri: &str) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("open").arg(uri).status();
        }

        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("cmd").args(&["/c", "start", "", uri]).status();
        }

        #[cfg(target_os = "linux")]
        {
            let _ = Command::new("xdg-open").arg(uri).status();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ide_bridge_uri_building() {
        let vscode_uri = IdeBridge::build_uri(Ide::VSCode, "/src/main.rs", Some(42), Some(10));
        assert_eq!(vscode_uri, "vscode://file/src/main.rs:42:10");

        let cursor_uri = IdeBridge::build_uri(Ide::Cursor, "/src/lib.rs", Some(15), None);
        assert_eq!(cursor_uri, "cursor://file/src/lib.rs:15");

        let webstorm_uri = IdeBridge::build_uri(Ide::WebStorm, "/src/App.tsx", Some(100), Some(5));
        assert_eq!(webstorm_uri, "webstorm://open?file=/src/App.tsx&line=100&column=5");
    }
}
