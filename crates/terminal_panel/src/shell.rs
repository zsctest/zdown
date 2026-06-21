//! 跨平台 Shell 检测。

/// 检测当前平台的默认 shell。
///
/// 返回 (shell 程序路径, 命令行参数)。
pub fn detect_shell() -> (String, Vec<String>) {
    if cfg!(target_os = "windows") {
        // Windows: 优先使用 PowerShell
        let pwsh = which("pwsh.exe").or_else(|_| which("powershell.exe"));
        match pwsh {
            Ok(path) => (path, Vec::new()),
            Err(_) => (String::from("cmd.exe"), Vec::new()),
        }
    } else {
        // Unix: 使用 $SHELL 环境变量
        if let Ok(shell) = std::env::var("SHELL") {
            return (shell, Vec::new());
        }
        // Fallback 尝试常见 shell
        for sh in &["/bin/zsh", "/bin/bash", "/bin/sh"] {
            if std::path::Path::new(sh).exists() {
                return (sh.to_string(), Vec::new());
            }
        }
        (String::from("/bin/sh"), Vec::new())
    }
}

/// 在 PATH 中查找可执行文件。
fn which(name: &str) -> Result<String, ()> {
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let full = dir.join(name);
            if full.is_file() {
                return Ok(full.to_string_lossy().into_owned());
            }
        }
    }
    Err(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_shell_returns_program() {
        let (program, _args) = detect_shell();
        assert!(!program.is_empty());
    }
}
