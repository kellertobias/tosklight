//! The audio input a Media Server listens to: opening it, reporting what it hears, and opening it
//! late once macOS grants microphone access after startup.

use media_application::MediaConfiguration;

use crate::SharedConfiguration;

/// The open audio input, if one is. Shared, because granting microphone access after startup
/// opens a stream into the analysis the outputs already read.
pub(crate) type SharedAudio = std::sync::Arc<std::sync::Mutex<Option<media_audio::AudioService>>>;

/// Opens the configured audio input, and the analysis whatever happened publishes into.
///
/// Audio capture is a real capability of the machine: when there is no input device the server
/// says so once and runs on silence, rather than refusing to start.
pub(crate) fn start(
    configuration: &MediaConfiguration,
) -> (SharedAudio, media_audio::SharedAnalysis) {
    let analysis = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
        media_audio::AnalysisSnapshot::default(),
    ));
    let audio = match media_audio::AudioService::start_bounded_publishing(
        &configuration.audio,
        analysis.clone(),
    ) {
        Ok(service) => Some(service),
        Err(error) => {
            tracing::warn!(%error, "no audio input; generated sources will run on silence");
            None
        }
    };
    (std::sync::Arc::new(std::sync::Mutex::new(audio)), analysis)
}

pub(crate) fn permission_view(
    status: media_audio::permission::MicrophonePermission,
) -> media_http::wire::MicrophonePermissionView {
    use media_audio::permission::MicrophonePermission as Native;
    use media_http::wire::MicrophonePermissionView as View;
    match status {
        Native::NotRequired => View::NotRequired,
        Native::NotDetermined => View::NotDetermined,
        Native::Denied => View::Denied,
        Native::Restricted => View::Restricted,
        Native::Granted => View::Granted,
    }
}

/// What the Audio page's meters and lamps read.
pub(crate) fn telemetry(
    audio: &SharedAudio,
    analysis: &media_audio::SharedAnalysis,
) -> media_http::AudioSource {
    let audio = audio.clone();
    let analysis = analysis.clone();
    std::sync::Arc::new(move || {
        let Ok(guard) = audio.lock() else {
            return media_http::AudioTelemetry::default();
        };
        let Some(service) = guard.as_ref() else {
            return media_http::AudioTelemetry::default();
        };
        let heard = analysis.load();
        media_http::AudioTelemetry {
            capturing: true,
            device: service.device().to_owned(),
            detail: None,
            waveform: heard.analysis.waveform.clone(),
            spectrum: heard.analysis.spectrum.clone(),
            bass: heard.analysis.bass,
            mid: heard.analysis.mid,
            treble: heard.analysis.treble,
            energy: heard.analysis.energy,
            peak: heard.analysis.peak,
            beat: heard.beat,
            bpm: heard.bpm,
            beat_phase: heard.beat_phase,
            tempo_confidence: heard.tempo_confidence,
            kick_level: heard.instruments.kick.level,
            kick_hit: heard.instruments.kick.hit,
            snare_level: heard.instruments.snare.level,
            snare_hit: heard.instruments.snare.hit,
            hihat_level: heard.instruments.hihat.level,
            hihat_hit: heard.instruments.hihat.hit,
            gain: heard.gain,
            clipping: heard.clipping,
        }
    })
}

/// Asks macOS for microphone access and, once it is granted, opens the configured input.
pub(crate) fn permission_request(
    audio: &SharedAudio,
    analysis: &media_audio::SharedAnalysis,
    live: &SharedConfiguration,
) -> std::sync::Arc<
    dyn Fn() -> Result<media_http::wire::MicrophonePermissionView, String> + Send + Sync,
> {
    let audio = audio.clone();
    let analysis = analysis.clone();
    let live = live.clone();
    std::sync::Arc::new(move || {
        let status = media_audio::permission::request()?;
        if status == media_audio::permission::MicrophonePermission::Granted {
            let mut service = audio.lock().map_err(|error| error.to_string())?;
            if service.is_none() {
                let configuration = live.load();
                let started = media_audio::AudioService::start_bounded_publishing(
                    &configuration.audio,
                    analysis.clone(),
                )
                .map_err(|error| error.to_string())?;
                *service = Some(started);
            }
        }
        Ok(permission_view(status))
    })
}
