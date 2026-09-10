//! Export exactly the bytes served by the running application's download routes.
fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let destination = std::path::PathBuf::from(args.next().expect("destination directory"));
    let options: Vec<_> = args.collect();
    let check = options.iter().any(|arg| arg == "--check");
    let all = options.iter().any(|arg| arg == "--all");
    for (name, bytes) in media_application::gdtf::packages()? {
        // Only MagicQ assets are checked into the repository; other formats are server downloads.
        if !all && !name.ends_with(".hed") && !name.ends_with(".csv") {
            continue;
        }
        let path = destination.join(name);
        if check {
            assert_eq!(std::fs::read(&path)?, bytes, "{} is stale", path.display());
        } else {
            std::fs::create_dir_all(&destination)?;
            std::fs::write(path, bytes)?;
        }
    }
    Ok(())
}
