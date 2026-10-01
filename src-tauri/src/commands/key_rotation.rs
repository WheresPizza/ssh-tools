//! Reviewable key replacement. No keys are deleted and no remote access is attempted.
use super::{profiles, repositories, ssh_config, ssh_keys};
use crate::error::{AppError, Result};
use crate::utils::{permissions, ssh_dir};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    io::Write,
    path::{Path, PathBuf},
};
fn invalid(s: impl Into<String>) -> AppError {
    AppError::InvalidInput(s.into())
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RotationTarget {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub source: String,
    pub line: usize,
    pub before: String,
    pub after: String,
    pub blocked: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RotationPlan {
    pub old_path: String,
    pub old_fingerprint: String,
    pub new_path: String,
    pub new_fingerprint: String,
    pub revisions: BTreeMap<String, String>,
    pub targets: Vec<RotationTarget>,
    pub warnings: Vec<String>,
}
fn quote(path: &str) -> String {
    format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
}
fn check_key(path: &str, fingerprint: &str) -> Result<()> {
    ssh_dir::validate_key_path(path)?;
    let key = ssh_keys::parse_key_info(Path::new(path))?;
    if key.fingerprint != fingerprint {
        return Err(invalid(
            "A key changed. Reload the library and preview replacement again.",
        ));
    }
    Ok(())
}
fn discover(
    old_path: String,
    old_fingerprint: String,
    new_path: String,
    new_fingerprint: String,
) -> Result<RotationPlan> {
    check_key(&old_path, &old_fingerprint)?;
    check_key(&new_path, &new_fingerprint)?;
    if old_fingerprint == new_fingerprint {
        return Err(invalid(
            "Choose a different key identity, not another copy of the same key",
        ));
    }
    if new_path
        .chars()
        .any(|c| c.is_control() || c == '%' || c == '$')
    {
        return Err(invalid(
            "Replacement key path contains SSH expansion characters",
        ));
    }
    let root = ssh_dir::get_ssh_dir()?;
    let main = ssh_dir::get_ssh_config_path()?;
    let documents = ssh_config::config_documents()?;
    let mut revisions = BTreeMap::new();
    let mut targets = Vec::new();
    let mut warnings = vec!["Register the new public key with every affected service before switching. This wizard does not install keys or test authentication. Old key files, agent enrollment and annotations are kept unchanged.".into(), "Only explicit references to this key file are covered. Default identities, repository SSH commands, certificates, environment overrides and remote installations require separate review.".into()];
    // Validate the generated section before offering any changes.
    let profile_snapshot = profiles::list_blocking()?;
    for (path, content) in &documents {
        let source = path.to_string_lossy().to_string();
        revisions.insert(source.clone(), ssh_config::config_revision(content));
        let managed = ssh_dir::managed_path(&root, path).is_ok();
        let profile_lines = if path == &main {
            profiles::managed_line_count(content)
        } else {
            0
        };
        if path == &main {
            for p in profile_snapshot
                .profiles
                .iter()
                .filter(|p| p.key_path == old_path)
            {
                targets.push(RotationTarget {
                    id: format!("profile:{}", p.alias),
                    kind: "profile".into(),
                    label: format!("{} ({})", p.name, p.alias),
                    source: source.clone(),
                    line: 0,
                    before: p.key_path.clone(),
                    after: new_path.clone(),
                    blocked: (!managed)
                        .then(|| "Configuration is outside the writable SSH directory".into()),
                });
            }
        }
        let mut scope = "Global defaults".to_string();
        let mut ambiguous = true;
        for (index, line) in content.lines().enumerate() {
            let (directive, value) = ssh_config::directive(line);
            if directive.eq_ignore_ascii_case("host") {
                scope = value.to_string();
                ambiguous = value.split_whitespace().count() != 1
                    || !value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
            } else if directive.eq_ignore_ascii_case("match") {
                scope = format!("Match {value}");
                ambiguous = true;
            }
            if index < profile_lines || !directive.eq_ignore_ascii_case("identityfile") {
                continue;
            }
            let resolved = repositories::identity_path(value, &root);
            if resolved.as_deref() != Some(old_path.as_str()) {
                if resolved.is_none() && value != "none" {
                    warnings.push(format!(
                        "Unresolved IdentityFile at {source}:{} ({scope}); review manually.",
                        index + 1
                    ));
                }
                continue;
            }
            let reason = if !managed {
                Some("Included file is outside the writable SSH directory".into())
            } else if ambiguous {
                Some("Global, Match or patterned Host rules need manual review".into())
            } else {
                None
            };
            let (body, comment) = ssh_config::split_comment(line);
            let start = body.find(|c: char| !c.is_whitespace()).unwrap_or(0);
            let offset = body[start..]
                .find(|c: char| c.is_whitespace() || c == '=')
                .map(|i| start + i)
                .unwrap_or(body.len());
            let value_start = body[offset..]
                .find(|c: char| !c.is_whitespace() && c != '=')
                .map(|i| offset + i)
                .unwrap_or(body.len());
            let trailing = &body[body.trim_end().len()..];
            let after = format!(
                "{}{}{}{}",
                &body[..value_start],
                quote(&new_path),
                trailing,
                comment
            );
            targets.push(RotationTarget {
                id: format!("host:{source}:{index}"),
                kind: "host".into(),
                label: scope.clone(),
                source: source.clone(),
                line: index + 1,
                before: line.to_string(),
                after,
                blocked: reason,
            });
        }
    }
    Ok(RotationPlan {
        old_path,
        old_fingerprint,
        new_path,
        new_fingerprint,
        revisions,
        targets,
        warnings,
    })
}
#[tauri::command(rename_all = "snake_case")]
pub async fn preview_key_rotation(
    old_path: String,
    old_fingerprint: String,
    new_path: String,
    new_fingerprint: String,
) -> Result<RotationPlan> {
    tauri::async_runtime::spawn_blocking(move || {
        discover(old_path, old_fingerprint, new_path, new_fingerprint)
    })
    .await
    .map_err(|e| invalid(e.to_string()))?
}
fn edits(
    plan: &RotationPlan,
    selected: &[String],
    documents: &[(PathBuf, String)],
) -> Result<Vec<(PathBuf, String, String)>> {
    let ids: HashSet<_> = selected.iter().collect();
    if ids.is_empty() || ids.len() != selected.len() {
        return Err(invalid("Select at least one distinct reference"));
    }
    for id in &ids {
        if !plan
            .targets
            .iter()
            .any(|t| &t.id == *id && t.blocked.is_none())
        {
            return Err(invalid(
                "Selected reference is unavailable or requires manual review",
            ));
        }
    }
    let main = ssh_dir::get_ssh_config_path()?;
    let mut result = Vec::new();
    for (path, original) in documents {
        let targets: Vec<_> = plan
            .targets
            .iter()
            .filter(|t| ids.contains(&t.id) && Path::new(&t.source) == path)
            .collect();
        if targets.is_empty() {
            continue;
        }
        ssh_config::resolve_config_path(&path.to_string_lossy())?;
        // Line edits preserve all unrelated bytes and line endings.
        let mut updated = String::new();
        for (index, line) in original.split_inclusive('\n').enumerate() {
            if let Some(t) = targets
                .iter()
                .find(|t| t.kind == "host" && t.line == index + 1)
            {
                updated.push_str(&t.after);
                if line.ends_with("\r\n") {
                    updated.push_str("\r\n");
                } else if line.ends_with('\n') {
                    updated.push('\n');
                }
            } else {
                updated.push_str(line);
            }
        }
        if path == &main && targets.iter().any(|t| t.kind == "profile") {
            let (mut entries, tail) = profiles::split(&updated)?;
            for p in &mut entries {
                if ids.contains(&format!("profile:{}", p.alias)) {
                    p.key_path = plan.new_path.clone();
                }
            }
            updated = profiles::render(&entries)? + tail;
        }
        result.push((path.clone(), original.clone(), updated));
    }
    Ok(result)
}
// Stage replacements, rollback copies and standard recovery backups before modifying
// any config. Rename each file atomically; on failure restore already-written files.
fn commit_files(changes: &[(PathBuf, String, String)], fail_at: Option<usize>) -> Result<()> {
    struct Staged {
        path: PathBuf,
        next: tempfile::NamedTempFile,
        rollback: tempfile::NamedTempFile,
        before: String,
        after: String,
    }
    fn stage(path: &Path, text: &str) -> Result<tempfile::NamedTempFile> {
        let mut f = tempfile::NamedTempFile::new_in(
            path.parent().ok_or_else(|| invalid("Missing parent"))?,
        )?;
        f.write_all(text.as_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.as_file()
                .set_permissions(std::fs::Permissions::from_mode(
                    std::fs::metadata(path)?.permissions().mode() & 0o600,
                ))?;
        }
        f.as_file().sync_all()?;
        Ok(f)
    }
    let mut staged = Vec::new();
    let mut backups = Vec::new();
    for (path, before, after) in changes {
        permissions::reject_symlink(path)?;
        permissions::reject_symlink(path.parent().unwrap())?;
        if std::fs::read_to_string(path)? != *before {
            return Err(invalid("SSH config changed; preview again"));
        }
        let backup = permissions::backup_path(path);
        permissions::reject_symlink(&backup)?;
        if backup.exists() && !backup.is_file() {
            return Err(invalid("Recovery backup path is not a regular file"));
        }
        backups.push((backup, stage(path, before)?));
        staged.push(Staged {
            path: path.clone(),
            next: stage(path, after)?,
            rollback: stage(path, before)?,
            before: before.clone(),
            after: after.clone(),
        });
    }
    for (path, backup) in backups {
        backup.persist(path).map_err(|e| AppError::Io(e.error))?;
    }
    let mut applied: Vec<(PathBuf, tempfile::NamedTempFile, String)> = Vec::new();
    for (index, item) in staged.into_iter().enumerate() {
        let operation = (|| {
            permissions::reject_symlink(&item.path)?;
            if std::fs::read_to_string(&item.path)? != item.before {
                return Err(invalid("SSH config changed during replacement"));
            }
            if fail_at == Some(index) {
                return Err(invalid("Simulated write failure"));
            }
            item.next
                .persist(&item.path)
                .map_err(|e| AppError::Io(e.error))?;
            Ok(())
        })();
        if let Err(error) = operation {
            let mut unrestored = Vec::new();
            for (path, rollback, after) in applied.into_iter().rev() {
                if path.is_symlink()
                    || std::fs::read_to_string(&path).ok().as_ref() != Some(&after)
                    || rollback.persist(&path).is_err()
                {
                    unrestored.push(path.display().to_string());
                }
            }
            if unrestored.is_empty() {
                return Err(invalid(format!(
                    "Replacement failed; applied changes were rolled back: {error}"
                )));
            }
            return Err(invalid(format!("Replacement failed: {error}. Could not roll back {}; review Settings → File recovery before continuing.", unrestored.join(", "))));
        }
        applied.push((item.path, item.rollback, item.after));
    }
    for (path, _, _) in changes {
        std::fs::File::open(path.parent().unwrap()).and_then(|f| f.sync_all()).map_err(|e| invalid(format!("Replacement was written, but directory synchronization failed: {e}. Review the configuration and Settings → File recovery before retrying.")))?;
    }
    Ok(())
}
fn apply(plan: RotationPlan, selected: Vec<String>) -> Result<()> {
    let _guard = crate::utils::SSH_WRITE_LOCK
        .lock()
        .map_err(|_| invalid("SSH write lock unavailable"))?;
    let current = discover(
        plan.old_path.clone(),
        plan.old_fingerprint.clone(),
        plan.new_path.clone(),
        plan.new_fingerprint.clone(),
    )?;
    if current != plan {
        return Err(invalid(
            "Keys or SSH configuration changed. Preview replacement again.",
        ));
    }
    let documents = ssh_config::config_documents()?;
    if documents.iter().any(|(p, c)| {
        plan.revisions.get(&p.to_string_lossy().to_string())
            != Some(&ssh_config::config_revision(c))
    }) {
        return Err(invalid("SSH config changed; preview again"));
    }
    commit_files(&edits(&plan, &selected, &documents)?, None)
}
#[tauri::command(rename_all = "snake_case")]
pub async fn apply_key_rotation(plan: RotationPlan, selected: Vec<String>) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || apply(plan, selected))
        .await
        .map_err(|e| invalid(e.to_string()))?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_keeps_backups_and_rolls_back_earlier_files_on_failure() {
        let d = tempfile::tempdir().unwrap();
        let changes: Vec<_> = ["a", "b"]
            .into_iter()
            .map(|name| {
                let path = d.path().join(name);
                std::fs::write(&path, name).unwrap();
                (path, name.to_string(), format!("new-{name}"))
            })
            .collect();
        assert!(commit_files(&changes, Some(1))
            .unwrap_err()
            .to_string()
            .contains("rolled back"));
        for (p, b, _) in &changes {
            assert_eq!(std::fs::read_to_string(p).unwrap(), *b);
            assert_eq!(
                std::fs::read_to_string(permissions::backup_path(p)).unwrap(),
                *b
            );
        }
        commit_files(&changes, None).unwrap();
        for (p, b, a) in &changes {
            assert_eq!(std::fs::read_to_string(p).unwrap(), *a);
            assert_eq!(
                std::fs::read_to_string(permissions::backup_path(p)).unwrap(),
                *b
            );
        }
    }
    #[test]
    fn preflight_failure_does_not_modify_any_config() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a");
        let b = d.path().join("b");
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();
        std::fs::create_dir(permissions::backup_path(&b)).unwrap();
        assert!(commit_files(
            &[
                (a.clone(), "a".into(), "new".into()),
                (b.clone(), "b".into(), "new".into())
            ],
            None
        )
        .is_err());
        assert_eq!(std::fs::read_to_string(a).unwrap(), "a");
        assert_eq!(std::fs::read_to_string(b).unwrap(), "b");
    }
}
