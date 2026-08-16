#![forbid(unsafe_code)]
//! The standalone ToskLight visualizer.
//!
//! This process owns its own window, its own connection to a scene source, and its own network
//! DMX receivers. It never launches, supervises, or depends on the ToskLight desktop, and its
//! failure cannot affect the desk's server, output engine, or user interface.

mod app;
#[cfg(test)]
mod demo;
mod effects;
mod embedded;
mod helper_source;
mod lasers;
mod physics;

mod media_worker;
/// How many slots the renderer divides a gobo wheel into.
mod menu;
mod pacing;
mod planner;
mod png;
mod redraw;
mod session;
mod settings;
mod showfile;
mod snapshots;
mod ui;

use settings::Options;
use winit::event_loop::{ControlFlow, EventLoop};

/// The event loop, with the activation policy this launch should have.
#[cfg(target_os = "macos")]
fn build_event_loop(accessory: bool) -> Result<EventLoop<()>, winit::error::EventLoopError> {
    use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
    let mut builder = EventLoop::builder();
    if accessory {
        builder.with_activation_policy(ActivationPolicy::Accessory);
    }
    builder.build()
}

#[cfg(not(target_os = "macos"))]
fn build_event_loop(_accessory: bool) -> Result<EventLoop<()>, winit::error::EventLoopError> {
    EventLoop::builder().build()
}

/// The embedded pane: answer the desk, then draw its Stage until the channel ends.
fn run_embedded() -> i32 {
    let source = match helper_source::HelperSource::start(
        std::io::stdin(),
        std::io::stdout(),
        "viz-renderer".to_owned(),
    ) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("the desk's channel: {error}");
            return 1;
        }
    };
    match embedded::run(source) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("the embedded pane: {error}");
            1
        }
    }
}

fn main() {
    let options = match Options::from_arguments(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n\n{}", Options::usage());
            std::process::exit(2);
        }
    };
    if options.help {
        println!("{}", Options::usage());
        return;
    }

    // An embedded pane has no window at all: the desk owns the window, the pane is a rectangle
    // inside it, and the chrome above the picture is the desk's own web interface. So this launch
    // never reaches the window system — no event loop, no activation policy, nothing to activate.
    if options.embed {
        std::process::exit(run_embedded());
    }

    // A renderer launched by the desk or the PreViz editor is their window, not another
    // application. On macOS it therefore uses an accessory activation policy: it draws and takes
    // key input, but stays out of the Dock and App Switcher. A directly launched PreViz renderer
    // owns the product's one regular application identity.
    let event_loop = match build_event_loop(accessory_process(
        options.helper,
        std::env::var("TOSKLIGHT_VIZ_LAUNCHED_BY").ok().as_deref(),
    )) {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("window system: {error}");
            std::process::exit(1);
        }
    };
    // Present the first frame straight away. From then on the application paces itself to the
    // display and waits for the next frame to fall due, so DMX keeps arriving on screen without
    // the event loop sitting inside the driver where input cannot reach it.
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut application = app::Application::new(options);
    if let Err(error) = event_loop.run_app(&mut application) {
        eprintln!("window system: {error}");
        std::process::exit(1);
    }
}

fn accessory_process(helper: bool, launched_by: Option<&str>) -> bool {
    helper || matches!(launched_by, Some("desk" | "editor"))
}

#[cfg(test)]
mod identity_tests {
    use super::accessory_process;

    #[test]
    fn companion_renderers_are_accessories_but_direct_previz_is_regular() {
        assert!(accessory_process(false, Some("editor")));
        assert!(accessory_process(false, Some("desk")));
        assert!(accessory_process(true, None));
        assert!(!accessory_process(false, None));
    }
}
