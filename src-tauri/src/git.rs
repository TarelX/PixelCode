use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

pub(crate) struct GitService {
    root: PathBuf,
}

pub(crate) struct Worktree {
    root: PathBuf,
    pub path: PathBuf,
    snapshot: String,
}

fn command(root: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(root);
    command
}

fn output(command: &mut Command) -> Result<Vec<u8>, String> {
    let result = command.output().map_err(|e| format!("无法运行 git：{e}"))?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).trim().into());
    }
    Ok(result.stdout)
}

impl GitService {
    pub fn detect(root: &Path) -> Option<Self> {
        let inside = output(command(root).args(["rev-parse", "--is-inside-work-tree"])).ok()?;
        if inside != b"true\n" && inside != b"true\r\n" {
            return None;
        }
        let top = output(command(root).args(["rev-parse", "--show-toplevel"])).ok()?;
        let top = PathBuf::from(String::from_utf8_lossy(&top).trim());
        let root = root.canonicalize().ok()?;
        (top.canonicalize().ok()? == root).then_some(Self { root })
    }

    pub fn worktree(&self, allowed: &Path, camp: &str, task: &str) -> Result<Worktree, String> {
        for id in [camp, task] {
            if id.is_empty() || id.contains(['/', '\\', ':']) || id.contains("..") || id == "." {
                return Err("worktree 所属大营或任务 ID 无效".into());
            }
        }
        fs::create_dir_all(allowed).map_err(|e| e.to_string())?;
        let allowed = allowed.canonicalize().map_err(|e| e.to_string())?;
        let parent = crate::artifacts::resolve(&allowed, Path::new(camp))?;
        fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
        let parent = parent.canonicalize().map_err(|e| e.to_string())?;
        if !parent.starts_with(&allowed) {
            return Err("worktree 路径越出允许目录".into());
        }
        let path = parent.join(task);
        if path.symlink_metadata().is_ok() {
            return Err("worktree 路径已存在".into());
        }
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let index = allowed.join(format!(
            ".index-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let snapshot = (|| {
            let head = output(command(&self.root).args(["rev-parse", "--verify", "HEAD"])).ok();
            if let Some(head) = &head {
                output(
                    command(&self.root)
                        .env("GIT_INDEX_FILE", &index)
                        .arg("read-tree")
                        .arg(String::from_utf8_lossy(head).trim()),
                )?;
            }
            output(
                command(&self.root)
                    .env("GIT_INDEX_FILE", &index)
                    .args(["add", "-A"]),
            )?;
            let tree = output(
                command(&self.root)
                    .env("GIT_INDEX_FILE", &index)
                    .arg("write-tree"),
            )?;
            let mut commit = command(&self.root);
            commit.args([
                "-c",
                "user.name=PixelCode",
                "-c",
                "user.email=pixelcode@localhost",
                "commit-tree",
            ]);
            commit.arg(String::from_utf8_lossy(&tree).trim());
            if let Some(head) = head {
                commit.arg("-p").arg(String::from_utf8_lossy(&head).trim());
            }
            commit.args(["-m", "PixelCode task snapshot"]);
            output(&mut commit).map(|bytes| String::from_utf8_lossy(&bytes).trim().to_string())
        })();
        let _ = fs::remove_file(&index);
        let snapshot = snapshot?;
        output(
            command(&self.root)
                .args(["worktree", "add", "--detach"])
                .arg(&path)
                .arg(&snapshot),
        )?;
        Ok(Worktree {
            root: self.root.clone(),
            path,
            snapshot,
        })
    }

    pub fn apply(&self, patch: &[u8], reverse: bool) -> Result<(), String> {
        if patch.is_empty() {
            return Ok(());
        }
        let mut cmd = command(&self.root);
        cmd.args(["apply", "--binary"]);
        if reverse {
            cmd.arg("-R");
        }
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let write = child
            .stdin
            .take()
            .ok_or("无法写入 git 补丁")?
            .write_all(patch);
        let result = child.wait_with_output().map_err(|e| e.to_string())?;
        if !result.status.success() {
            return Err(String::from_utf8_lossy(&result.stderr).trim().into());
        }
        write.map_err(|e| e.to_string())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn directory(label: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!(
                "{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).unwrap();
        path.canonicalize().unwrap()
    }

    pub fn git(root: &Path, args: &[&str]) -> Vec<u8> {
        output(command(root).args(args)).unwrap()
    }

    pub fn repo(root: &Path) {
        git(root, &["init"]);
        git(root, &["config", "core.autocrlf", "false"]);
        fs::write(root.join("base.txt"), "original\n").unwrap();
        git(root, &["add", "-A"]);
        git(
            root,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@localhost",
                "commit",
                "-m",
                "initial",
            ],
        );
    }

    pub fn cleanup(path: &Path) {
        #[cfg(windows)]
        fn writable(path: &Path) {
            if path.is_dir() {
                for entry in fs::read_dir(path).unwrap() {
                    writable(&entry.unwrap().path());
                }
            } else {
                let mut permissions = fs::metadata(path).unwrap().permissions();
                permissions.set_readonly(false);
                fs::set_permissions(path, permissions).unwrap();
            }
        }
        #[cfg(windows)]
        writable(path);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn snapshot_preserves_index_refs_and_dirty_untracked_files() {
        let root = directory("snapshot");
        let trees = directory("snapshot-trees");
        repo(&root);
        fs::write(root.join("base.txt"), "staged\n").unwrap();
        git(&root, &["add", "-A"]);
        fs::write(root.join("base.txt"), "unstaged\n").unwrap();
        fs::write(root.join("new.txt"), "new\n").unwrap();
        fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(root.join("ignored.txt"), "ignored\n").unwrap();
        let index = git(&root, &["diff", "--cached", "--binary"]);
        let head = git(&root, &["rev-parse", "HEAD"]);
        let service = GitService::detect(&root).unwrap();
        let tree = service.worktree(&trees, "camp", "task").unwrap();
        assert!(tree.path.starts_with(&trees));
        assert_eq!(
            fs::read_to_string(tree.path.join("base.txt")).unwrap(),
            "unstaged\n"
        );
        assert!(tree.path.join("new.txt").is_file());
        assert!(!tree.path.join("ignored.txt").exists());
        assert!(tree.patch().unwrap().is_empty());
        assert_eq!(index, git(&root, &["diff", "--cached", "--binary"]));
        assert_eq!(head, git(&root, &["rev-parse", "HEAD"]));
        let path = tree.path.clone();
        drop(tree);
        assert!(!path.exists());
        assert!(service.worktree(&trees, "../escape", "task").is_err());
        assert!(service.worktree(&trees, "camp", "../escape").is_err());
        assert!(GitService::detect(&root.join(".git")).is_none());
        cleanup(&root);
        cleanup(&trees);
    }

    #[test]
    fn unborn_repo_snapshots_and_non_git_fallback() {
        let root = directory("unborn");
        let trees = directory("unborn-trees");
        assert!(GitService::detect(&root).is_none());
        git(&root, &["init"]);
        git(&root, &["config", "core.autocrlf", "false"]);
        fs::write(root.join("new.txt"), "new").unwrap();
        let service = GitService::detect(&root).unwrap();
        let tree = service.worktree(&trees, "camp", "task").unwrap();
        assert!(tree.path.join("new.txt").is_file());
        fs::write(tree.path.join("new.txt"), "changed").unwrap();
        let patch = tree.patch().unwrap();
        service.apply(&patch, false).unwrap();
        assert_eq!(fs::read_to_string(root.join("new.txt")).unwrap(), "changed");
        service.apply(&patch, true).unwrap();
        assert_eq!(fs::read_to_string(root.join("new.txt")).unwrap(), "new");
        assert!(service.apply(b"invalid patch", false).is_err());
        drop(tree);
        cleanup(&root);
        cleanup(&trees);
    }
}

impl Worktree {
    pub fn patch(&self) -> Result<Vec<u8>, String> {
        output(command(&self.path).args(["add", "-A"]))?;
        output(command(&self.path).args(["diff", "--cached", "--binary", &self.snapshot]))
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        let _ = output(
            command(&self.root)
                .args(["worktree", "remove", "--force"])
                .arg(&self.path),
        );
        let _ = output(command(&self.root).args(["worktree", "prune"]));
    }
}
