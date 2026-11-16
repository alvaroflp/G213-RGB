//! XDG desktop autostart for the current graphical login.

use std::{fs, io, path::PathBuf};

fn entry_path() -> PathBuf {
    crate::config::config_dir()
        .with_file_name("autostart")
        .join("g213-rgb.desktop")
}

pub fn enabled() -> bool {
    fs::read_to_string(entry_path()).is_ok_and(|entry| {
        entry.lines().any(|line| line.starts_with("Exec="))
            && !entry.lines().any(|line| {
                matches!(
                    line.trim(),
                    "Hidden=true" | "X-GNOME-Autostart-enabled=false"
                )
            })
    })
}

fn desktop_entry(executable: &str) -> io::Result<String> {
    if executable.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "caminho inválido para inicialização",
        ));
    }
    // Exec field quoting, followed by the desktop entry string escaping layer.
    let mut exec = String::new();
    for ch in executable.chars() {
        match ch {
            '%' => exec.push_str("%%"),
            '\\' | '"' | '$' | '`' => {
                exec.push('\\');
                exec.push(ch);
            }
            _ => exec.push(ch),
        }
    }
    let exec = exec.replace('\\', "\\\\");
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=G213 RGB\nExec=\"{exec}\" --background\nIcon=g213-rgb\nTerminal=false\n"
    ))
}

pub fn set_enabled(enable: bool) -> io::Result<()> {
    let path = entry_path();
    if !enable {
        return match fs::remove_file(path) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        };
    }
    let executable = std::env::current_exe()?;
    let executable = executable.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "o caminho do aplicativo precisa estar em UTF-8",
        )
    })?;
    let entry = desktop_entry(executable)?;
    fs::create_dir_all(path.parent().expect("autostart directory"))?;
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&temporary, entry)?;
    fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn desktop_exec_quotes_path_without_shell_expansion() {
        let entry = super::desktop_entry("/home/usuário/Área de trabalho/100%/g213-rgb").unwrap();
        assert!(
            entry.contains("Exec=\"/home/usuário/Área de trabalho/100%%/g213-rgb\" --background\n")
        );
        assert!(super::desktop_entry("/bad\npath").is_err());
    }
}
