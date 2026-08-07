#![forbid(unsafe_code)]
//! The standalone ToskLight visualizer.
//!
//! This process owns its own window, its own connection to a scene source, and its own network
//! DMX receivers. It never launches, supervises, or depends on the ToskLight desktop, and its
//! failure cannot affect the desk's server, output engine, or user interface.

mod app;
mod demo;
mod helper_source;
mod lasers;

/// How many slots the renderer divides a gobo wheel into.
mod menu;
mod pacing;
mod planner;
mod png;
mod session;
mod settings;
mod showfile;
mod snapshots;
mod ui;

use settings::Options;
use winit::event_loop::{ControlFlow, EventLoop};

/// The event loop, with the activation policy this launch should have.
#[cfg(target_os = "macos")]
fn build_event_loop(helper: bool) -> Result<EventLoop<()>, winit::error::EventLoopError> {
    use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
    let mut builder = EventLoop::builder();
    if helper {
        builder.with_activation_policy(ActivationPolicy::Accessory);
    }
    builder.build()
}

#[cfg(not(target_os = "macos"))]
fn build_event_loop(_helper: bool) -> Result<EventLoop<()>, winit::error::EventLoopError> {
    EventLoop::builder().build()
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

    // A helper is the desk's window, not a second application. On macOS that means an accessory
    // activation policy: it draws, it takes key input, and it is absent from the Dock and the App
    // Switcher, so an operator sees one ToskLight rather than two. The standalone visualizer is a
    // product in its own right and keeps its tile.
    let event_loop = match build_event_loop(options.helper) {
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
