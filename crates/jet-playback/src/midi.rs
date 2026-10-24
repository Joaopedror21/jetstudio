//! MIDI input [Manual: envsettings_midi]. Every input port is opened ("Enable device"); notes
//! play the target channel (the selected one) live through a dedicated queue into the audio
//! thread, so MIDI does not wait for a UI frame.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// A live note for the audio thread. `channel` 0 means no target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveNote {
    On { channel: u64, key: u8, velocity: u8 },
    Off { channel: u64, key: u8 },
}

/// Parse one MIDI message. Note-on with velocity 0 is a note-off (MIDI 1.0 convention).
pub fn parse(bytes: &[u8], channel: u64) -> Option<LiveNote> {
    let status = *bytes.first()?;
    let key = *bytes.get(1)?;
    let velocity = bytes.get(2).copied().unwrap_or(0);
    if key > 127 {
        return None;
    }
    match status & 0xF0 {
        0x90 if velocity > 0 => Some(LiveNote::On {
            channel,
            key,
            velocity: velocity.min(127),
        }),
        0x90 | 0x80 => Some(LiveNote::Off { channel, key }),
        _ => None,
    }
}

/// A note as it arrived, with the output device frame at that moment (where the live note
/// sounds), for recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StampedNote {
    pub note: LiveNote,
    pub frame: u64,
}

type Queues = (rtrb::Producer<LiveNote>, rtrb::Producer<StampedNote>);

/// MIDI input connections; dropping it closes them. "Input device - Select the device in the
/// Input list"; "Enable device - ... FL Studio will now receive MIDI from that device" [Manual:
/// envsettings_midi]. Every device starts enabled (our choice: FL starts with none).
pub struct MidiInputs {
    connections: Vec<midir::MidiInputConnection<()>>,
    /// Names of the connected (enabled) ports.
    pub names: Vec<String>,
    /// Every input port found, enabled or not.
    pub available: Vec<String>,
    /// Channel id that incoming notes play (0 = none).
    pub target: Arc<AtomicU64>,
    /// Every note played, for note recording; drain it each UI frame.
    pub log: rtrb::Consumer<StampedNote>,
    sink: Arc<Mutex<Queues>>,
    frames: Arc<AtomicU64>,
}

impl MidiInputs {
    /// Connect every available input port. Notes go into `sink` (shared by all ports; the lock
    /// is taken on MIDI threads only, never on the audio thread).
    pub fn connect_all(
        sink: rtrb::Producer<LiveNote>,
        frames: Arc<AtomicU64>,
    ) -> Result<Self, String> {
        let (log_sink, log) = rtrb::RingBuffer::new(1024);
        let mut inputs = Self {
            connections: Vec::new(),
            names: Vec::new(),
            available: Vec::new(),
            target: Arc::new(AtomicU64::new(0)),
            log,
            sink: Arc::new(Mutex::new((sink, log_sink))),
            frames,
        };
        inputs.connect(&[])?;
        Ok(inputs)
    }

    /// (Re)connect every input port except those named in `disabled`.
    pub fn connect(&mut self, disabled: &[String]) -> Result<(), String> {
        self.connections.clear();
        self.names.clear();
        let probe = midir::MidiInput::new("JetStudio").map_err(|e| e.to_string())?;
        let ports = probe.ports();
        self.available = ports
            .iter()
            .map(|port| {
                probe
                    .port_name(port)
                    .unwrap_or_else(|_| "MIDI input".to_owned())
            })
            .collect();
        for (port, name) in ports.iter().zip(self.available.clone()) {
            if disabled.contains(&name) {
                continue;
            }
            let Ok(input) = midir::MidiInput::new("JetStudio") else {
                continue;
            };
            let sink = Arc::clone(&self.sink);
            let target = Arc::clone(&self.target);
            let frames = Arc::clone(&self.frames);
            let callback = move |_stamp: u64, bytes: &[u8], _: &mut ()| {
                let channel = target.load(Ordering::Relaxed);
                if channel == 0 {
                    return;
                }
                if let Some(note) = parse(bytes, channel)
                    && let Ok(mut queues) = sink.lock()
                {
                    let _ = queues.0.push(note);
                    let frame = frames.load(Ordering::Acquire);
                    let _ = queues.1.push(StampedNote { note, frame });
                }
            };
            if let Ok(connection) = input.connect(port, "jetstudio-in", callback, ()) {
                self.connections.push(connection);
                self.names.push(name);
            }
        }
        Ok(())
    }

    pub fn set_target(&self, channel: Option<u64>) {
        self.target.store(channel.unwrap_or(0), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_note_on_off_and_running_zero_velocity() {
        assert_eq!(
            parse(&[0x90, 60, 100], 3),
            Some(LiveNote::On {
                channel: 3,
                key: 60,
                velocity: 100
            })
        );
        assert_eq!(
            parse(&[0x93, 60, 0], 3),
            Some(LiveNote::Off {
                channel: 3,
                key: 60
            })
        );
        assert_eq!(
            parse(&[0x80, 61, 64], 3),
            Some(LiveNote::Off {
                channel: 3,
                key: 61
            })
        );
        assert_eq!(parse(&[0xB0, 7, 100], 3), None, "controllers are not notes");
        assert_eq!(parse(&[0x90], 3), None, "truncated messages are ignored");
    }
}
