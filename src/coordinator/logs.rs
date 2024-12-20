const LOG_DIR: &str = "/logs";

use crate::config;
use std::fs::{self, DirEntry, File};
use std::io::{self, Write};
use std::path::Path;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use coordinator::LogInfo;

pub fn add_log(package_name: &str, log_content: &[String]) -> io::Result<()> {
    let log_dir = Path::new(LOG_DIR);
    if !log_dir.exists() {
        fs::create_dir_all(log_dir)?;
    }

    // Generate a unique file name with a timestamp
    let timestamp = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());
    let log_file_name = format!("{timestamp}_{package_name}.log");
    let log_file_path = log_dir.join(log_file_name);

    // Write the log content to the file
    let mut log_file = File::create(&log_file_path)?;
    log_file.write_all(log_content.join("").as_bytes())?;

    // Remove the oldest log if the directory exceeds maximum. Skipped if max logs is zero
    if config::max_logs() == 0 {
        let mut log_entries: Vec<_> = fs::read_dir(log_dir)?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .collect();

        log_entries.sort_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok());
        if log_entries.len() > config::max_logs() as usize {
            let logs_to_delete = log_entries.len() - config::max_logs() as usize;
            for entry in log_entries.iter().take(logs_to_delete) {
                fs::remove_file(entry.path())?;
            }
        }
    }

    Ok(())
}

pub fn list_logs() -> io::Result<Vec<LogInfo>> {
    let log_dir = Path::new(LOG_DIR);
    if !log_dir.exists() {
        return Ok(vec![]);
    }

    let log_files: Vec<LogInfo> = fs::read_dir(log_dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| parse_log_file_name(&entry))
        .collect();

    Ok(log_files)
}

fn parse_log_file_name(entry: &DirEntry) -> Option<LogInfo> {
    entry.file_name().into_string().ok().and_then(|file_name| {
        if let Some((time, package)) = file_name.split_once('_') {
            let package = package.trim_end_matches(".log").to_string();
            Some(LogInfo {
                package,
                time: time.to_string(),
            })
        } else {
            None
        }
    })
}

pub fn get_log_by_index(index: usize) -> io::Result<Option<String>> {
    let log_dir = Path::new(LOG_DIR);

    if !log_dir.exists() {
        return Ok(None);
    }

    let mut log_entries: Vec<_> = fs::read_dir(log_dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .collect();

    // Sort log files in ascending order of modification time
    log_entries.sort_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok());

    if index >= log_entries.len() {
        return Ok(None);
    }

    let log_file_path = log_entries[index].path();
    let content = fs::read_to_string(log_file_path)?;
    Ok(Some(content))
}
