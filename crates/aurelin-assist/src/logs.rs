use std::{
    fs,
    io::{Read, Seek, SeekFrom},
};

use anyhow::{Result, bail};
use aurelin_framework::telemetry::{LOG_DIRECTORY, LOG_PREFIX};
use serde_json::Value;

use crate::project::Project;

const MAX_BYTES: u64 = 1024 * 1024;

/// Read at most 1 MiB from the newest seven daily files. Incomplete records at
/// either boundary are ignored; they may be from a concurrent writer.
pub fn recent(project: &Project, count: usize, errors_only: bool) -> Result<Vec<Value>> {
    if !(1..=100).contains(&count) {
        bail!("count must be between 1 and 100");
    }
    let directory = project.root().join(LOG_DIRECTORY);
    if !directory.try_exists()? {
        bail!("no backend logs; enable ASSIST_LOG=true and start the application");
    }
    let mut files =
        fs::read_dir(directory)?.map(|entry| entry.map(|entry| entry.path())).collect::<Result<Vec<_>, _>>()?;
    files.retain(|path| {
        path.file_name().and_then(|name| name.to_str()).is_some_and(|name| {
            name.strip_prefix(&format!("{LOG_PREFIX}."))
                .is_some_and(|date| date.len() == 10 && date.bytes().all(|byte| byte.is_ascii_digit() || byte == b'-'))
        })
    });
    files.sort();
    let mut remaining = MAX_BYTES;
    let mut entries = Vec::new();
    for path in files.iter().rev().take(7) {
        if remaining == 0 || entries.len() == count {
            break;
        }
        let mut file = fs::File::open(project.existing_file(path)?)?;
        let length = file.metadata()?.len();
        let start = length.saturating_sub(remaining);
        file.seek(SeekFrom::Start(start))?;
        let mut bytes = Vec::new();
        file.take(remaining).read_to_end(&mut bytes)?;
        remaining = remaining.saturating_sub(bytes.len() as u64);
        let mut lines = bytes.split(|byte| *byte == b'\n');
        if start != 0 {
            lines.next();
        }
        let mut records = lines.collect::<Vec<_>>();
        // JSONL requires a trailing newline; don't return a partial last write.
        records.pop();
        for line in records.into_iter().rev() {
            let Ok(entry) = serde_json::from_slice::<Value>(line) else { continue };
            if !errors_only || entry["level"].as_str() == Some("ERROR") {
                entries.push(entry);
                if entries.len() == count {
                    break;
                }
            }
        }
    }
    entries.reverse();
    Ok(entries)
}
