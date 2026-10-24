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

/// Open MIDI input connections; dropping it closes them.
pub struct MidiInputs {
    _connections: Vec<midir::MidiInputConnection<()>>,
    pub names: Vec<String>,
    /// Channel id that incoming notes play (0 = none).
    pub target: Arc<AtomicU64>,
}

impl MidiInputs {
    /// Connect every available input port. Notes go into `sink` (shared by all ports; the lock
    /// is taken on MIDI threads only, never on the audio thread).
    pub fn connect_all(sink: rtrb::Producer<LiveNote>) -> Result<Self, String> {
        let target = Arc::new(AtomicU64::new(0));
        let sink = Arc::new(Mutex::new(sink));
        let probe = midir::MidiInput::new("JetStudio").map_err(|e| e.to_string())?;
        let ports = probe.ports();
        let mut connections = Vec::new();
        let mut names = Vec::new();
        for port in &ports {
            let name = probe
                .port_name(port)
                .unwrap_or_else(|_| "MIDI input".to_owned());
            let Ok(input) = midir::MidiInput::new("JetStudio") else {
                continue;
            };
            let sink = Arc::clone(&sink);
            let target = Arc::clone(&target);
            let callback = move |_stamp: u64, bytes: &[u8], _: &mut ()| {
                let channel = target.load(Ordering::Relaxed);
                if channel == 0 {
                    return;
                }
                if let Some(note) = parse(bytes, channel)
                    && let Ok(mut queue) = sink.lock()
                {
                    let _ = queue.push(note);
                }
            };
            if let Ok(connection) = input.connect(port, "jetstudio-in", callback, ()) {
                connections.push(connection);
                names.push(name);
            }
        }
        Ok(Self {
            _connections: connections,
            names,
            target,
        })
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
