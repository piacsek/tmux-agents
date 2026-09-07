use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub stdout: String,
    pub ok: bool,
}

pub fn run(
    cache_dir: &Path,
    ttl: Duration,
    command: &[String],
    now: SystemTime,
    runner: &mut dyn FnMut(&[String]) -> io::Result<Run>,
) -> io::Result<String> {
    let file = cache_file(cache_dir, command);
    if let Some(cached) = fresh(&file, ttl, now) {
        return Ok(cached);
    }
    let run = runner(command)?;
    let output = run.stdout.trim_end_matches('\n').to_string();
    if run.ok {
        store(&file, &output, now)?;
    }
    Ok(output)
}

fn fresh(file: &Path, ttl: Duration, now: SystemTime) -> Option<String> {
    let modified = fs::metadata(file).ok()?.modified().ok()?;
    let age = now.duration_since(modified).ok()?;
    (age < ttl).then(|| fs::read_to_string(file).ok())?
}

fn store(file: &Path, output: &str, now: SystemTime) -> io::Result<()> {
    let dir = file
        .parent()
        .ok_or_else(|| io::Error::other("no cache dir"))?;
    private_dir(dir)?;
    let tmp = file.with_extension(format!("tmp{}", std::process::id()));
    let mut handle = fs::File::create(&tmp)?;
    handle.write_all(output.as_bytes())?;
    handle.set_modified(now)?;
    fs::rename(&tmp, file)
}

pub fn private_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

fn cache_file(cache_dir: &Path, command: &[String]) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    command.hash(&mut hasher);
    cache_dir.join(format!("{:016x}", hasher.finish()))
}

pub fn shell_out(command: &[String]) -> io::Result<Run> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| io::Error::other("empty command"))?;
    let output = std::process::Command::new(program)
        .args(args)
        .stderr(std::process::Stdio::null())
        .output()?;
    Ok(Run {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        ok: output.status.success(),
    })
}
