//! An isolated capture must not replace the retained, already presented Program.
//! These exercise the production compositor without acquiring another window frame between
//! layer captures and the next Program readback, as happens when presentation is skipped.

use media_domain::geometry::Size;
use media_domain::{
    LayerState, MasterState, MediaAddress, OutputId, ScalingMode, SourceStatus, Timestamp, Tint,
};
use media_render::{Compositor, Gpu, LayerDraw, OffScreenOutput, PROGRAM_FORMAT, SourceTexture};

const SIZE: Size = Size::new(32, 32);

fn ready(state: LayerState) -> LayerState {
    LayerState {
        address: MediaAddress::new(3, 1),
        source_status: SourceStatus::Ready,
        scaling_mode: ScalingMode::Stretch,
        ..state
    }
}

struct Bench {
    gpu: Gpu,
    compositor: Compositor,
    output: OutputId,
    presented: OffScreenOutput,
}

impl Bench {
    fn new() -> Self {
        let gpu = Gpu::off_screen().expect("preview isolation requires a GPU or software adapter");
        Self {
            compositor: Compositor::new(&gpu, SIZE, PROGRAM_FORMAT),
            output: OutputId::new(),
            presented: OffScreenOutput::new(&gpu, SIZE),
            gpu,
        }
    }

    fn solid(&self, color: [u8; 4]) -> SourceTexture {
        SourceTexture::solid(&self.gpu, Size::new(4, 4), color).unwrap()
    }

    fn present(&mut self, draws: &[LayerDraw<'_>], master: &MasterState) {
        self.compositor.render(
            draws,
            master,
            None,
            self.presented.view(),
            self.output,
            Timestamp::ZERO,
            None,
        );
    }

    fn program(&mut self, size: Size, master: &MasterState) -> Vec<u8> {
        let target = OffScreenOutput::new(&self.gpu, size);
        self.compositor
            .render_master_into(master, None, target.view());
        target.read_image()
    }

    fn layer(&mut self, size: Size, draw: LayerDraw<'_>, now: Timestamp) -> Vec<u8> {
        let target = OffScreenOutput::new(&self.gpu, size);
        self.compositor
            .render_layer_preview(draw, target.view(), self.output, now);
        target.read_image()
    }
}

#[test]
fn eight_isolated_layers_ending_in_black_alpha_do_not_replace_retained_program() {
    let mut bench = Bench::new();
    let colors = [
        [40, 100, 220, 255],
        [240, 50, 10, 255],
        [20, 230, 90, 255],
        [180, 40, 200, 255],
        [80, 180, 170, 255],
        [200, 180, 30, 255],
        [220, 100, 130, 255],
        [255, 255, 255, 128],
    ];
    let sources: Vec<_> = colors.into_iter().map(|color| bench.solid(color)).collect();
    let mut states: Vec<_> = (0..8).map(|_| ready(LayerState::default())).collect();
    for (index, state) in states.iter_mut().enumerate().take(7).skip(1) {
        state.scale_x = 0.3;
        state.scale_y = 0.3;
        state.position_x = (index % 3) as f32 * 0.6 - 0.6;
        state.position_y = (index / 3) as f32 * 0.6 - 0.3;
    }
    states[7].tint = Tint::new(0.0, 0.0, 0.0);
    states[7].dimmer = 0.32;
    let draws: Vec<_> = states
        .iter()
        .zip(&sources)
        .map(|(state, source)| LayerDraw {
            state,
            source,
            mask: None,
        })
        .collect();
    let master = MasterState::default();
    bench.present(&draws, &master);
    let expected = bench.program(SIZE, &master);
    assert!(expected.chunks_exact(4).any(|pixel| pixel[2] > 0));
    for draw in &draws {
        let isolated = bench.layer(SIZE, *draw, Timestamp::ZERO);
        if std::ptr::eq(draw.state, &states[7]) {
            assert!(
                isolated
                    .chunks_exact(4)
                    .all(|pixel| { pixel[..3] == [0, 0, 0] && pixel[3] > 0 && pixel[3] < 255 })
            );
        }
    }
    // Intentionally no present/render of the full composition here.
    let actual = bench.program(SIZE, &master);
    assert_eq!(
        actual, expected,
        "Program readback after isolated previews must retain the full colored composition"
    );
    assert_eq!(bench.presented.read_image(), expected);
}

#[test]
fn isolated_alpha_and_program_master_survive_repeated_capture_sizes_and_resize() {
    let mut bench = Bench::new();
    let background = bench.solid([40, 120, 220, 255]);
    let alpha_source = bench.solid([240, 80, 20, 128]);
    let background_state = ready(LayerState::default());
    let alpha_state = ready(LayerState {
        dimmer: 0.5,
        ..Default::default()
    });
    let isolated_draw = LayerDraw {
        state: &alpha_state,
        source: &alpha_source,
        mask: None,
    };
    let draws = [
        LayerDraw {
            state: &background_state,
            source: &background,
            mask: None,
        },
        isolated_draw,
    ];
    let master = MasterState {
        dimmer: 0.6,
        tint: Tint::new(0.8, 1.0, 0.7),
        ..Default::default()
    };
    for canvas in [SIZE, Size::new(48, 24), SIZE] {
        bench.compositor.resize(canvas);
        bench.presented.resize(canvas);
        bench.present(&draws, &master);
        for thumbnail in [Size::new(16, 8), Size::new(24, 16)] {
            let expected = bench.program(thumbnail, &master);
            let isolated = bench.layer(thumbnail, isolated_draw, Timestamp::ZERO);
            assert!(isolated.chunks_exact(4).all(|pixel| {
                pixel[3] > 0 && pixel[3] < 255 && pixel[0] > pixel[1] && pixel[1] > pixel[2]
            }));
            assert_eq!(
                bench.layer(thumbnail, isolated_draw, Timestamp::ZERO),
                isolated,
                "isolated alpha and color must remain exact across repeated captures"
            );
            assert_eq!(
                bench.program(thumbnail, &master),
                expected,
                "the retained Program and its master must not become the isolated layer"
            );
        }
    }
}

#[test]
fn an_unlit_layer_preview_cannot_clear_retained_program() {
    let mut bench = Bench::new();
    let source = bench.solid([20, 100, 200, 255]);
    let state = ready(LayerState::default());
    let draw = LayerDraw {
        state: &state,
        source: &source,
        mask: None,
    };
    let master = MasterState::default();
    bench.present(&[draw], &master);
    let expected = bench.program(SIZE, &master);
    let unlit = ready(LayerState {
        strobe_hz: Some(2.0),
        ..Default::default()
    });
    let preview = bench.layer(
        SIZE,
        LayerDraw {
            state: &unlit,
            source: &source,
            mask: None,
        },
        Timestamp::from_millis(300),
    );
    assert!(preview.iter().all(|byte| *byte == 0));
    assert_eq!(bench.program(SIZE, &master), expected);
}
