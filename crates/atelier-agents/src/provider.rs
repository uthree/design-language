//! The coding-agent CLIs an agent can run on, and how hard it is asked to think.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

/// A coding-agent CLI driven as a subprocess.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Anthropic's `claude`.
    ClaudeCode,
    /// OpenAI's `codex`.
    Codex,
    /// Google's Antigravity CLI, `agy`.
    Antigravity,
    /// The open-source `opencode`, which itself fronts many model vendors.
    OpenCode,
}

impl ProviderKind {
    /// Every provider, in the order a settings page lists them.
    pub const ALL: [Self; 4] = [
        Self::ClaudeCode,
        Self::Codex,
        Self::Antigravity,
        Self::OpenCode,
    ];

    /// The name a person reads.
    pub fn label(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::Antigravity => "Antigravity CLI",
            Self::OpenCode => "OpenCode",
        }
    }

    /// The executable looked up on the host's `PATH`.
    pub fn executable(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude",
            Self::Codex => "codex",
            Self::Antigravity => "agy",
            Self::OpenCode => "opencode",
        }
    }

    /// Where the CLI's executable is on this machine: on `PATH`, or in `~/.local/bin`, where
    /// Claude Code's and Antigravity's installers put it. On Windows only a native `.exe`
    /// counts, never an npm `.cmd` shim, which cannot carry a JSON stream through `cmd.exe`.
    pub fn locate(self) -> Option<PathBuf> {
        find_on_path(self.executable()).or_else(|| {
            let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
            let name = if cfg!(windows) {
                format!("{}.exe", self.executable())
            } else {
                self.executable().to_owned()
            };
            Some(PathBuf::from(home).join(".local").join("bin").join(name))
                .filter(|path| path.is_file())
        })
    }

    /// The shell script that installs the CLI on a machine that lacks it, as each project
    /// documents it.
    pub fn install_script(self) -> &'static str {
        match self {
            Self::ClaudeCode => "curl -fsSL https://claude.ai/install.sh | bash",
            Self::Codex => "npm install -g @openai/codex",
            Self::Antigravity => "curl -fsSL https://antigravity.google/install.sh | bash",
            Self::OpenCode => "curl -fsSL https://opencode.ai/install | bash",
        }
    }

    /// The shell script that brings an installed CLI up to its latest release.
    pub fn update_script(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude update",
            Self::Codex => "npm install -g @openai/codex@latest",
            // Its installer replaces an existing install with the latest release.
            Self::Antigravity => self.install_script(),
            Self::OpenCode => "opencode upgrade",
        }
    }

    /// The script for `action`.
    pub fn setup_script(self, action: SetupAction) -> &'static str {
        match action {
            SetupAction::Install => self.install_script(),
            SetupAction::Update => self.update_script(),
        }
    }

    /// What a person runs in a terminal on the host to sign the CLI in to their account. Each
    /// CLI asks for it interactively, so an application shows it rather than running it.
    pub fn sign_in(self) -> &'static str {
        match self {
            // Its first run asks to sign in.
            Self::ClaudeCode => "claude",
            Self::Codex => "codex login",
            Self::Antigravity => "agy",
            Self::OpenCode => "opencode auth login",
        }
    }

    /// Model names the CLI itself documents as stable aliases that follow its newest models.
    /// Only Claude Code publishes such aliases; for the others, ask the CLI or a catalog.
    pub fn known_models(self) -> &'static [&'static str] {
        match self {
            Self::ClaudeCode => &["opus", "sonnet", "haiku"],
            Self::Codex | Self::Antigravity | Self::OpenCode => &[],
        }
    }
}

/// What to do to a provider's CLI on a host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupAction {
    /// Install it.
    Install,
    /// Update it to the latest release.
    Update,
}

/// Whether a provider's CLI is installed on a host, and which release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderStatus {
    /// The provider.
    pub provider: ProviderKind,
    /// Where its executable is, when it is on the host's `PATH`.
    pub path: Option<String>,
    /// What `--version` printed, first line only.
    pub version: Option<String>,
    /// An install or update running now.
    pub setup: Option<SetupAction>,
}

impl ProviderStatus {
    /// Whether the CLI was found.
    pub fn installed(&self) -> bool {
        self.path.is_some()
    }

    /// Looks for `provider` on this machine and asks it for its version. `program` overrides
    /// the executable, as [`crate::AgentConfig::program`] does. This runs a process: keep it off
    /// a UI thread.
    pub fn check(provider: ProviderKind, program: Option<&str>) -> Self {
        let path = match program {
            Some(program) if Path::new(program).components().count() > 1 => {
                Some(PathBuf::from(program)).filter(|path| path.is_file())
            }
            Some(program) => find_on_path(program),
            None => provider.locate(),
        };
        let version = path.as_ref().and_then(|path| {
            let output = Command::new(path)
                .arg("--version")
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&output.stdout);
            let line = text.lines().next()?.trim();
            (output.status.success() && !line.is_empty()).then(|| line.to_owned())
        });
        Self {
            provider,
            path: path.map(|path| path.to_string_lossy().into_owned()),
            version,
            setup: None,
        }
    }
}

/// `program` in a directory on `PATH`, with `.exe` on Windows.
fn find_on_path(program: &str) -> Option<PathBuf> {
    let name = if cfg!(windows) && Path::new(program).extension().is_none() {
        format!("{program}.exe")
    } else {
        program.to_owned()
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join(&name))
        .find(|path| path.is_file())
}

/// Reasoning effort requested of a model that supports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
    /// As little reasoning as the provider allows (Codex's `minimal`; `low` elsewhere).
    Minimal,
    /// Answer quickly.
    Low,
    /// The provider's default.
    Medium,
    /// Think longer before acting.
    High,
    /// Above high (`xhigh`), where the provider has such a level; `high` elsewhere.
    XHigh,
    /// The most the provider offers.
    Max,
}

impl Effort {
    /// The name a person reads.
    pub fn label(self) -> &'static str {
        match self {
            Self::Minimal => "Minimal effort",
            Self::Low => "Low effort",
            Self::Medium => "Medium effort",
            Self::High => "High effort",
            Self::XHigh => "Extra high effort",
            Self::Max => "Max effort",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_program_is_not_installed() {
        let status = ProviderStatus::check(
            ProviderKind::Codex,
            Some("/nonexistent/atelier-agents/codex"),
        );
        assert!(!status.installed());
        assert_eq!(status.version, None);
    }

    #[test]
    fn efforts_have_labels() {
        assert_eq!(Effort::XHigh.label(), "Extra high effort");
        assert_eq!(Effort::Minimal.label(), "Minimal effort");
    }
}
