//! The process an adapter asks for, as data.
//!
//! An adapter never starts anything itself. It names a program, arguments, a working directory
//! and environment, and the caller decides where that runs: [`CommandSpec::process`] runs it on
//! this machine, and [`CommandSpec::shell_line`] is the one line to hand a shell on another (over
//! `ssh`, in WSL or in a container), quoted for a POSIX shell.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

/// A process to start.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandSpec {
    /// The executable, looked up on `PATH` unless it is a path.
    pub program: String,
    /// Its arguments.
    pub args: Vec<String>,
    /// The directory it starts in.
    pub cwd: Option<PathBuf>,
    /// Variables to set.
    pub env: BTreeMap<String, String>,
    /// Variables to clear, so an agent never inherits another agent's runtime.
    pub env_remove: Vec<String>,
}

impl CommandSpec {
    /// A command with no arguments.
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            ..Self::default()
        }
    }

    /// Appends one argument.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Appends several arguments.
    pub fn args<I: IntoIterator<Item = S>, S: Into<String>>(mut self, args: I) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Sets the working directory.
    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Sets an environment variable.
    pub fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(name.into(), value.into());
        self
    }

    /// Clears an environment variable.
    pub fn env_remove(mut self, name: impl Into<String>) -> Self {
        self.env_remove.push(name.into());
        self
    }

    /// The process that runs this command on this machine. Standard streams are left to the
    /// caller.
    pub fn process(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        for name in &self.env_remove {
            command.env_remove(name);
        }
        command.envs(&self.env);
        command
    }

    /// The command as one line for a POSIX shell, with its directory and environment, for
    /// running it on another machine: `ssh -T host -- <line>`, `wsl.exe --exec sh -c <line>` or
    /// `docker exec -i container sh -c <line>`.
    pub fn shell_line(&self) -> String {
        let mut words = Vec::new();
        if let Some(cwd) = &self.cwd {
            words.push(format!("cd {} &&", quote(&cwd.to_string_lossy())));
        }
        // `env` sets and clears variables without depending on the remote shell's syntax.
        if !self.env.is_empty() || !self.env_remove.is_empty() {
            words.push("env".into());
            for name in &self.env_remove {
                words.push(format!("-u {}", quote(name)));
            }
            for (name, value) in &self.env {
                words.push(quote(&format!("{name}={value}")));
            }
        }
        words.push(quote(&self.program));
        words.extend(self.args.iter().map(|arg| quote(arg)));
        words.join(" ")
    }
}

/// Quotes a word so a POSIX shell passes it through unchanged. Plain words are left alone, so a
/// logged command line stays readable.
pub fn quote(word: &str) -> String {
    if !word.is_empty()
        && word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-=:,@+%".contains(&b))
    {
        return word.into();
    }
    format!("'{}'", word.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_only_what_needs_quoting() {
        assert_eq!(quote("claude"), "claude");
        assert_eq!(quote("/opt/bin/claude"), "/opt/bin/claude");
        assert_eq!(quote("A=1"), "A=1");
        assert_eq!(quote("my agent"), "'my agent'");
        assert_eq!(quote("it's"), r"'it'\''s'");
        assert_eq!(quote("~/bin"), "'~/bin'");
        assert_eq!(quote("$HOME"), "'$HOME'");
        assert_eq!(quote(""), "''");
    }

    #[test]
    fn a_shell_line_carries_the_directory_and_environment() {
        let command = CommandSpec::new("codex")
            .arg("app-server")
            .cwd("/work/my project")
            .env("A", "1")
            .env_remove("CLAUDECODE");
        assert_eq!(
            command.shell_line(),
            "cd '/work/my project' && env -u CLAUDECODE A=1 codex app-server"
        );
    }

    #[test]
    fn a_local_process_gets_the_arguments_and_directory() {
        let command = CommandSpec::new("claude")
            .args(["--print", "hi"])
            .cwd("/tmp")
            .process();
        assert_eq!(command.get_program(), "claude");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args, ["--print", "hi"]);
        assert_eq!(
            command.get_current_dir(),
            Some(std::path::Path::new("/tmp"))
        );
    }
}
