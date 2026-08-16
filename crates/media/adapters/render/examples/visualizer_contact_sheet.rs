//! Renders every generated visualizer to a PNG, for looking at them.
//!
//! A development affordance, not a test: the tests assert that each visualizer draws and reacts,
//! which a machine can check. Whether one looks good is a judgement, and this is how a person
//! makes it. Output goes wherever the first argument says.

use std::path::PathBuf;

use media_domain::audio::{Analysis, BANDS, WAVEFORM_POINTS};
use media_domain::geometry::Size;
use media_domain::visualizer::{ALL_KINDS, VisualizerConfiguration};
use media_render::{Gpu, VisualizerFrame, VisualizerRenderer};

fn main() -> anyhow::Result<()> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| ".artifacts/tmp/visualizers".to_owned()),
    );
    std::fs::create_dir_all(&directory)?;

    let size = Size::new(640, 360);
    let gpu = Gpu::off_screen().map_err(|error| anyhow::anyhow!("{error}"))?;
    let mut renderer = VisualizerRenderer::new(&gpu, size);

    let analysis = Analysis {
        waveform: (0..WAVEFORM_POINTS)
            .map(|index| (index as f32 * 0.06).sin() * 0.8)
            .collect(),
        spectrum: (0..BANDS)
            .map(|index| {
                let position = index as f32 / BANDS as f32;
                (1.0 - position).powf(1.5) * (0.6 + 0.4 * (position * 20.0).sin())
            })
            .collect(),
        bass: 0.8,
        mid: 0.6,
        treble: 0.45,
        energy: 0.7,
        peak: 0.95,
    };
    let frame = VisualizerFrame {
        seconds: 3.7,
        analysis: &analysis,
        beat: 0.8,
        bpm: 128.0,
        beat_phase: 0.2,
    };

    for kind in ALL_KINDS {
        let parameters = VisualizerConfiguration::new(kind).parameters;
        let texture = renderer
            .render(0, kind, &parameters, &frame)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let pixels = texture
            .read_rgba8(&gpu)
            .map_err(|error| anyhow::anyhow!("{error}"))?;

        let name = kind.label().to_lowercase().replace(' ', "-");
        let path = directory.join(format!("{:03}-{name}.png", kind.type_id()));
        let file = std::fs::File::create(&path)?;
        let mut encoder = png::Encoder::new(file, size.width, size.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        println!("{}", path.display());
    }
    Ok(())
}
