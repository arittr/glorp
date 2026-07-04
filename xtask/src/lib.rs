use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessStep {
    pub program: String,
    pub args: Vec<String>,
    pub best_effort: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XtaskCommand {
    CompanionFresh { release: bool },
}

const USAGE: &str = "Usage: cargo xtask companion fresh [--release]";

pub fn parse_args<I, S>(args: I) -> Result<XtaskCommand, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args = args
        .into_iter()
        .map(|arg| arg.as_ref().to_string())
        .collect::<Vec<_>>();

    match args.as_slice() {
        [companion, fresh] if companion == "companion" && fresh == "fresh" => {
            Ok(XtaskCommand::CompanionFresh { release: false })
        }
        [companion, fresh, flag]
            if companion == "companion" && fresh == "fresh" && flag == "--release" =>
        {
            Ok(XtaskCommand::CompanionFresh { release: true })
        }
        [flag] if flag == "--help" || flag == "-h" => Err(USAGE.to_string()),
        [] => Err(USAGE.to_string()),
        _ => Err(format!("unknown xtask command\n\n{USAGE}")),
    }
}

pub fn companion_fresh_steps(release: bool) -> Vec<ProcessStep> {
    let profile = if release { "release" } else { "debug" };
    vec![
        ProcessStep {
            program: "node".to_string(),
            args: vec![
                "scripts/build-macos-companion-app.mjs".to_string(),
                "--profile".to_string(),
                profile.to_string(),
            ],
            best_effort: false,
        },
        ProcessStep {
            program: "osascript".to_string(),
            args: vec!["-e".to_string(), "quit app \"Glorp\"".to_string()],
            best_effort: true,
        },
        ProcessStep {
            program: "pkill".to_string(),
            args: vec!["-f".to_string(), "Glorp.app/Contents/MacOS".to_string()],
            best_effort: true,
        },
        ProcessStep {
            program: "sleep".to_string(),
            args: vec!["1".to_string()],
            best_effort: true,
        },
        ProcessStep {
            program: "open".to_string(),
            args: vec!["target/macos/Glorp.app".to_string()],
            best_effort: false,
        },
    ]
}

pub fn run_xtask<I, S>(args: I, repo_root: &Path) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    match parse_args(args)? {
        XtaskCommand::CompanionFresh { release } => {
            if std::env::consts::OS != "macos" {
                return Err("cargo xtask companion fresh is only supported on macOS".to_string());
            }
            run_steps(&companion_fresh_steps(release), repo_root)
        }
    }
}

pub fn run_steps(steps: &[ProcessStep], repo_root: &Path) -> Result<(), String> {
    for step in steps {
        println!("xtask: {} {}", step.program, step.args.join(" "));
        let mut command = Command::new(&step.program);
        command.args(&step.args).current_dir(repo_root);
        if step.best_effort {
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
        } else {
            command
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
        }

        let status = command.status().map_err(|err| {
            format!(
                "failed to run `{}` from {}: {err}",
                step.program,
                repo_root.display()
            )
        })?;
        if !status.success() && !step.best_effort {
            return Err(format!(
                "`{} {}` failed with {status}",
                step.program,
                step.args.join(" ")
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn companion_fresh_builds_debug_bundle_by_default() {
        let steps = companion_fresh_steps(false);
        assert_eq!(
            steps.first(),
            Some(&ProcessStep {
                program: "node".to_string(),
                args: vec![
                    "scripts/build-macos-companion-app.mjs".to_string(),
                    "--profile".to_string(),
                    "debug".to_string(),
                ],
                best_effort: false,
            })
        );
    }

    #[test]
    fn companion_fresh_uses_release_profile_when_requested() {
        let steps = companion_fresh_steps(true);
        assert_eq!(
            steps.first(),
            Some(&ProcessStep {
                program: "node".to_string(),
                args: vec![
                    "scripts/build-macos-companion-app.mjs".to_string(),
                    "--profile".to_string(),
                    "release".to_string(),
                ],
                best_effort: false,
            })
        );
    }

    #[test]
    fn companion_fresh_relaunches_the_fresh_bundle() {
        let steps = companion_fresh_steps(false);
        assert_eq!(
            steps.last(),
            Some(&ProcessStep {
                program: "open".to_string(),
                args: vec!["target/macos/Glorp.app".to_string()],
                best_effort: false,
            })
        );
        assert!(steps
            .iter()
            .any(|step| step.program == "osascript" && step.best_effort));
        assert!(steps
            .iter()
            .any(|step| step.program == "pkill" && step.best_effort));
    }

    #[test]
    fn parses_companion_fresh() {
        assert_eq!(
            parse_args(["companion", "fresh"]),
            Ok(XtaskCommand::CompanionFresh { release: false })
        );
    }

    #[test]
    fn parses_companion_fresh_release() {
        assert_eq!(
            parse_args(["companion", "fresh", "--release"]),
            Ok(XtaskCommand::CompanionFresh { release: true })
        );
    }

    #[test]
    fn rejects_unknown_command_with_usage_hint() {
        let err = parse_args(["nope"]).unwrap_err();
        assert!(err.contains("Usage: cargo xtask companion fresh"));
    }
}
