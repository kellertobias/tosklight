#[cfg(feature = "native-midi")]
use crate::MidiTimecodeDecoder;
use crate::{ControlEvent, ControlInput};
use async_trait::async_trait;
#[cfg(feature = "native-midi")]
use midir::{Ignore, MidiInput, MidiInputConnection};

#[cfg(feature = "native-midi")]
pub fn available_midi_inputs() -> Result<Vec<String>, String> {
    let input = MidiInput::new("Light discovery").map_err(|error| error.to_string())?;
    input
        .ports()
        .iter()
        .map(|port| input.port_name(port).map_err(|error| error.to_string()))
        .collect()
}

#[cfg(not(feature = "native-midi"))]
pub fn available_midi_inputs() -> Result<Vec<String>, String> {
    Ok(Vec::new())
}

#[cfg(feature = "native-midi")]
pub struct MidiControlInput {
    _connection: MidiInputConnection<()>,
    receiver: tokio::sync::mpsc::Receiver<ControlEvent>,
}

#[cfg(feature = "native-midi")]
impl MidiControlInput {
    pub fn open(port_name: &str) -> Result<Self, String> {
        let mut input = MidiInput::new("Light").map_err(|error| error.to_string())?;
        input.ignore(Ignore::None);
        let port = input
            .ports()
            .into_iter()
            .find(|port| input.port_name(port).is_ok_and(|name| name == port_name))
            .ok_or_else(|| format!("MIDI input '{port_name}' was not found"))?;
        let source = port_name.to_owned();
        let (sender, receiver) = tokio::sync::mpsc::channel(1_024);
        let mut mtc = MidiTimecodeDecoder::default();
        let connection = input
            .connect(
                &port,
                "light-input",
                move |_timestamp, message, _| {
                    let event = if message.len() >= 2 && message[0] == 0xf1 {
                        mtc.push_quarter_frame(message[1], &source)
                            .ok()
                            .flatten()
                            .map(ControlEvent::Timecode)
                    } else {
                        message.first().map(|status| ControlEvent::Midi {
                            status: *status,
                            data: message[1..].to_vec(),
                        })
                    };
                    if let Some(event) = event {
                        let _ = sender.try_send(event);
                    }
                },
                (),
            )
            .map_err(|error| error.to_string())?;
        Ok(Self {
            _connection: connection,
            receiver,
        })
    }
}

#[cfg(feature = "native-midi")]
#[async_trait]
impl ControlInput for MidiControlInput {
    async fn next_event(&mut self) -> Option<ControlEvent> {
        self.receiver.recv().await
    }
}

/// Placeholder used by portable builds that intentionally omit native USB-MIDI.
#[cfg(not(feature = "native-midi"))]
pub struct MidiControlInput;

#[cfg(not(feature = "native-midi"))]
impl MidiControlInput {
    pub fn open(_port_name: &str) -> Result<Self, String> {
        Err("native MIDI is unavailable in this portable build".to_owned())
    }
}

#[cfg(not(feature = "native-midi"))]
#[async_trait]
impl ControlInput for MidiControlInput {
    async fn next_event(&mut self) -> Option<ControlEvent> {
        None
    }
}
