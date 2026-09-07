use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub fn run(
    cache_dir: &Path,
    ttl: Duration,
    command: &[String],
    now: SystemTime,
    runner: &mut dyn FnMut(&[String]) -> io::Result<String>,
) -> io::Result<String> {
    let file = cache_file(cache_dir, command);
    if let Some(cached) = fresh(&file, ttl, now) {
        return Ok(cached);
    }
    let output = runner(command)?.trim_end_matches('\n').to_string();
    store(&file, &output, now)?;
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
    fs::create_dir_all(dir)?;
    let tmp = file.with_extension(format!("tmp{}", std::process::id()));
    let mut handle = fs::File::create(&tmp)?;
    handle.write_all(output.as_bytes())?;
    handle.set_modified(now)?;
    fs::rename(&tmp, file)
}

fn cache_file(cache_dir: &Path, command: &[String]) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    command.hash(&mut hasher);
    cache_dir.join(format!("{:016x}", hasher.finish()))
}

pub fn shell_out(command: &[String]) -> io::Result<String> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| io::Error::other("empty command"))?;
    let output = std::process::Command::new(program)
        .args(args)
        .stderr(std::process::Stdio::null())
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
