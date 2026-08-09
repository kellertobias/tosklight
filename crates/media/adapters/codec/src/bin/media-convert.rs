//! Recursively normalise a folder of authored media into playback-ready `.toskclip` files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SOURCE_EXTENSIONS: &[&str] = &[
    "avi", "bmp", "dpx", "exr", "flv", "gif", "jpeg", "jpg", "m4v", "mkv", "mov", "mp4", "mpeg",
    "mpg", "png", "tif", "tiff", "webm", "webp", "wmv",
];

fn main() {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 2 {
        eprintln!("usage: media-convert <input-folder> <output-folder>");
        std::process::exit(2);
    }

    if let Err(message) = run(Path::new(&arguments[0]), Path::new(&arguments[1])) {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

fn run(input: &Path, output: &Path) -> Result<(), String> {
    let input = input
        .canonicalize()
        .map_err(|error| format!("cannot read {}: {error}", input.display()))?;
    if !input.is_dir() {
        return Err(format!("{} is not a folder", input.display()));
    }
    std::fs::create_dir_all(output)
        .map_err(|error| format!("cannot create {}: {error}", output.display()))?;
    let output = output
        .canonicalize()
        .map_err(|error| format!("cannot use {}: {error}", output.display()))?;
    if input == output {
        return Err("input and output folders must be different".to_owned());
    }

    let jobs = plan(&input, &output)?;
    if jobs.is_empty() {
        return Err(format!(
            "{} contains no supported image or video files",
            input.display()
        ));
    }

    let total = jobs.len();
    println!("Converting {total} files to HAP Alpha .toskclip media");
    let mut converted = 0usize;
    let mut skipped = 0usize;
    let mut failures = Vec::new();

    for (index, (destination, source)) in jobs.into_iter().enumerate() {
        let relative = destination.strip_prefix(&output).unwrap_or(&destination);
        if destination.exists() {
            println!(
                "[{}/{}] skip {} (already exists)",
                index + 1,
                total,
                relative.display()
            );
            skipped += 1;
            continue;
        }
        if let Some(parent) = destination.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            failures.push(format!("{}: {error}", destination.display()));
            continue;
        }

        println!("[{}/{}] {}", index + 1, total, relative.display());
        let mut last_percent = 0u32;
        let result = media_codec::import(&source, &destination, &mut |progress| {
            match progress {
                media_codec::Progress::Started {
                    width,
                    height,
                    frames,
                } => println!(
                    "  {width}x{height}, {} frames",
                    frames.map_or_else(|| "unknown".to_owned(), |value| value.to_string())
                ),
                media_codec::Progress::Encoded { .. } => {
                    if let Some(fraction) = progress.fraction() {
                        let percent = (fraction * 100.0).floor() as u32;
                        if percent >= last_percent + 10 {
                            last_percent = percent;
                            println!("  {percent}%");
                        }
                    }
                }
                media_codec::Progress::Finished { frames, bytes } => println!(
                    "  wrote {frames} frames, {:.1} MB",
                    bytes as f64 / 1_000_000.0
                ),
            }
            true
        });

        match result {
            Ok(_) => converted += 1,
            Err(error) => failures.push(format!("{}: {error}", source.display())),
        }
    }

    println!("Finished: {converted} converted, {skipped} already present");
    if failures.is_empty() {
        Ok(())
    } else {
        for failure in &failures {
            eprintln!("  {failure}");
        }
        Err(format!("{} files failed", failures.len()))
    }
}

fn plan(input: &Path, output: &Path) -> Result<BTreeMap<PathBuf, PathBuf>, String> {
    let mut sources = Vec::new();
    collect(input, output, &mut sources)?;
    sources.sort();

    let mut jobs = BTreeMap::new();
    for source in sources {
        let relative = source
            .strip_prefix(input)
            .expect("collected source remains below input");
        let mut destination = output.join(relative);
        destination.set_extension("toskclip");
        if let Some(previous) = jobs.insert(destination.clone(), source.clone()) {
            return Err(format!(
                "{} and {} would both become {}; rename one source first",
                previous.display(),
                source.display(),
                destination.display()
            ));
        }
    }
    Ok(jobs)
}

fn collect(folder: &Path, output: &Path, sources: &mut Vec<PathBuf>) -> Result<(), String> {
    if folder == output {
        return Ok(());
    }
    let entries = std::fs::read_dir(folder)
        .map_err(|error| format!("cannot read {}: {error}", folder.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read directory entry: {error}"))?;
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect(&path, output, sources)?;
        } else if file_type.is_file() && is_source(&path) {
            sources.push(path);
        }
    }
    Ok(())
}

fn is_source(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            SOURCE_EXTENSIONS
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recursively_preserves_folders_and_replaces_extensions() {
        let root = scratch("tree");
        let input = root.join("input");
        let output = root.join("output");
        std::fs::create_dir_all(input.join("Loops/Blue")).unwrap();
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(input.join("Still.PNG"), b"source").unwrap();
        std::fs::write(input.join("Loops/Blue/Wave.mov"), b"source").unwrap();
        std::fs::write(input.join("README.txt"), b"ignore").unwrap();
        std::fs::create_dir_all(input.join(".thumbs")).unwrap();
        std::fs::write(input.join(".thumbs/Still.jpg"), b"generated").unwrap();

        let jobs = plan(&input, &output).unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(
            jobs.get(&output.join("Still.toskclip")),
            Some(&input.join("Still.PNG"))
        );
        assert_eq!(
            jobs.get(&output.join("Loops/Blue/Wave.toskclip")),
            Some(&input.join("Loops/Blue/Wave.mov"))
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_two_sources_that_have_the_same_destination() {
        let root = scratch("collision");
        let input = root.join("input");
        let output = root.join("output");
        std::fs::create_dir_all(&input).unwrap();
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(input.join("Look.png"), b"source").unwrap();
        std::fs::write(input.join("Look.mov"), b"source").unwrap();

        let error = plan(&input, &output).unwrap_err();
        assert!(error.contains("would both become"));

        std::fs::remove_dir_all(root).unwrap();
    }

    fn scratch(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tosklight-media-convert-{label}-{}",
            std::process::id()
        ))
    }
}
