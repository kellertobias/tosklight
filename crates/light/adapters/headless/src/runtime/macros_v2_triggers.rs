//! The two directions of one word: how a Macro was started.
//!
//! Split from the Macro routes because the list only ever grows — every new way to start a Macro
//! adds a line to each of these — and because a lookup table is not what a reader of the routes
//! is there for.

use light_wire::v2::macros as wire;

pub(super) fn application_trigger(
    trigger: wire::MacroTrigger,
) -> light_application::CommandMacroTrigger {
    match trigger {
        wire::MacroTrigger::Pool => light_application::CommandMacroTrigger::Pool,
        wire::MacroTrigger::Editor => light_application::CommandMacroTrigger::Editor,
        wire::MacroTrigger::Playback { playback_number } => {
            light_application::CommandMacroTrigger::Playback { playback_number }
        }
        wire::MacroTrigger::CommandLine => light_application::CommandMacroTrigger::CommandLine,
        wire::MacroTrigger::Http => light_application::CommandMacroTrigger::Http,
        wire::MacroTrigger::WebSocket => light_application::CommandMacroTrigger::WebSocket,
        wire::MacroTrigger::Osc => light_application::CommandMacroTrigger::Osc,
        wire::MacroTrigger::Hardware => light_application::CommandMacroTrigger::Hardware,
        wire::MacroTrigger::Schedule => light_application::CommandMacroTrigger::Schedule,
        wire::MacroTrigger::Timecode => light_application::CommandMacroTrigger::Timecode,
        wire::MacroTrigger::Tracking => light_application::CommandMacroTrigger::Tracking,
    }
}

pub(super) fn trigger_wire(trigger: light_application::CommandMacroTrigger) -> wire::MacroTrigger {
    match trigger {
        light_application::CommandMacroTrigger::Pool => wire::MacroTrigger::Pool,
        light_application::CommandMacroTrigger::Editor => wire::MacroTrigger::Editor,
        light_application::CommandMacroTrigger::Playback { playback_number } => {
            wire::MacroTrigger::Playback { playback_number }
        }
        light_application::CommandMacroTrigger::CommandLine => wire::MacroTrigger::CommandLine,
        light_application::CommandMacroTrigger::Http => wire::MacroTrigger::Http,
        light_application::CommandMacroTrigger::WebSocket => wire::MacroTrigger::WebSocket,
        light_application::CommandMacroTrigger::Osc => wire::MacroTrigger::Osc,
        light_application::CommandMacroTrigger::Hardware => wire::MacroTrigger::Hardware,
        light_application::CommandMacroTrigger::Schedule => wire::MacroTrigger::Schedule,
        light_application::CommandMacroTrigger::Timecode => wire::MacroTrigger::Timecode,
        light_application::CommandMacroTrigger::Tracking => wire::MacroTrigger::Tracking,
    }
}
