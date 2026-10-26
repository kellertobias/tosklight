use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let parent = std::env::var_os("LIGHT_TMP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/tmp")
            });
        let directory = parent.join(format!(
            "image-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn png(path: &Path, width: u32, height: u32) {
    let file = fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&vec![0; width as usize * height as usize * 4])
        .unwrap();
}

#[test]
fn publication_decodes_real_dimensions_preserves_existing_files_and_honors_cancel() {
    let scratch = Scratch::new();
    let staged = scratch.0.join("staged.png");
    let destination = scratch.0.join("operator.png");
    fs::write(&destination, b"user image must survive").unwrap();
    png(&staged, 3840, 2160);
    let cancel = AtomicBool::new(false);
    let image = publish_png(&staged, &destination, &cancel).unwrap();
    assert_eq!((image.width, image.height), (3840, 2160));
    assert_ne!(image.path, destination);
    assert_eq!(fs::read(&destination).unwrap(), b"user image must survive");
    assert!(
        staged.is_file(),
        "publisher never owns caller staging lifetime"
    );
    cancel.store(true, Ordering::Release);
    assert_eq!(
        publish_png(&staged, &destination, &cancel).unwrap_err(),
        ImageError::Cancelled
    );
    assert!(
        image.path.is_file(),
        "cancellation never deletes completed images"
    );
}

#[test]
fn invalid_truncated_and_non_4k_images_never_publish() {
    let scratch = Scratch::new();
    let staged = scratch.0.join("invalid.png");
    let destination = scratch.0.join("operator.png");
    let cancel = AtomicBool::new(false);
    png(&staged, 32, 24);
    assert!(publish_png(&staged, &destination, &cancel).is_err());
    png(&staged, 3840, 2160);
    let file = fs::OpenOptions::new().write(true).open(&staged).unwrap();
    file.set_len(64).unwrap();
    assert!(publish_png(&staged, &destination, &cancel).is_err());
    assert!(!destination.exists());
}
