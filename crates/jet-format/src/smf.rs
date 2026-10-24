//! Standard MIDI Files (SMF, type 0 and 1) for MIDI export/import [Manual:
//! fformats_save_export "MIDI - .mid saves Step Sequencer / Piano roll note data";
//! pianoroll_menu "Import MIDI file...", "Export as MIDI file..."]. Written from the public
//! MIDI 1.0 file specification; only notes, tempo and track names are kept.

use std::fs;
use std::path::Path;
use thiserror::Error;

/// Safety limit on imported files.
pub const MAX_SMF_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum SmfError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a Standard MIDI File: {0}")]
    Invalid(&'static str),
    #[error("MIDI file uses SMPTE time division, which is not supported")]
    Smpte,
    #[error("MIDI file is too large")]
    TooLarge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SmfNote {
    pub start: u64,
    pub length: u64,
    pub key: u8,
    pub velocity: u8,
    /// MIDI channel 0..=15.
    pub channel: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SmfTrack {
    pub name: String,
    pub notes: Vec<SmfNote>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Smf {
    /// Ticks per quarter note.
    pub ppq: u16,
    /// Microseconds per quarter note of the first tempo event (500 000 = 120 BPM).
    pub tempo_us: u32,
    pub tracks: Vec<SmfTrack>,
}

fn vlq(mut value: u64, out: &mut Vec<u8>) {
    let mut bytes = [0u8; 10];
    let mut n = 0;
    loop {
        bytes[n] = (value & 0x7F) as u8;
        value >>= 7;
        n += 1;
        if value == 0 {
            break;
        }
    }
    for i in (0..n).rev() {
        out.push(bytes[i] | if i > 0 { 0x80 } else { 0 });
    }
}

/// Serialize as SMF type 1: a tempo track followed by one track per [`SmfTrack`].
pub fn to_bytes(smf: &Smf) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&((smf.tracks.len() + 1) as u16).to_be_bytes());
    out.extend_from_slice(&smf.ppq.to_be_bytes());
    // Tempo track.
    let mut tempo = Vec::new();
    vlq(0, &mut tempo);
    tempo.extend_from_slice(&[0xFF, 0x51, 0x03]);
    tempo.extend_from_slice(&smf.tempo_us.to_be_bytes()[1..]);
    vlq(0, &mut tempo);
    tempo.extend_from_slice(&[0xFF, 0x2F, 0x00]);
    push_chunk(&mut out, &tempo);
    for track in &smf.tracks {
        let mut events: Vec<(u64, bool, SmfNote)> = Vec::new();
        for note in &track.notes {
            events.push((note.start, true, *note));
            events.push((note.start + note.length.max(1), false, *note));
        }
        // Note-offs before note-ons at the same tick so repeated keys retrigger.
        events.sort_by_key(|(tick, on, note)| (*tick, *on, note.key));
        let mut data = Vec::new();
        let name = track.name.as_bytes();
        vlq(0, &mut data);
        data.extend_from_slice(&[0xFF, 0x03]);
        vlq(name.len() as u64, &mut data);
        data.extend_from_slice(name);
        let mut last = 0;
        for (tick, on, note) in events {
            vlq(tick - last, &mut data);
            last = tick;
            let channel = note.channel & 0x0F;
            if on {
                data.extend_from_slice(&[
                    0x90 | channel,
                    note.key & 0x7F,
                    note.velocity.clamp(1, 127),
                ]);
            } else {
                data.extend_from_slice(&[0x80 | channel, note.key & 0x7F, 0x40]);
            }
        }
        vlq(0, &mut data);
        data.extend_from_slice(&[0xFF, 0x2F, 0x00]);
        push_chunk(&mut out, &data);
    }
    out
}

fn push_chunk(out: &mut Vec<u8>, data: &[u8]) {
    out.extend_from_slice(b"MTrk");
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(data);
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, SmfError> {
        let b = *self
            .data
            .get(self.pos)
            .ok_or(SmfError::Invalid("truncated data"))?;
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&[u8], SmfError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|end| *end <= self.data.len())
            .ok_or(SmfError::Invalid("truncated data"))?;
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn vlq(&mut self) -> Result<u64, SmfError> {
        let mut value = 0u64;
        for _ in 0..4 {
            let b = self.byte()?;
            value = (value << 7) | u64::from(b & 0x7F);
            if b & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(SmfError::Invalid("variable-length number too long"))
    }
}

/// Parse SMF type 0 or 1. Running status is supported; unknown chunks are skipped.
pub fn from_bytes(data: &[u8]) -> Result<Smf, SmfError> {
    let mut r = Reader { data, pos: 0 };
    if r.take(4)? != b"MThd" {
        return Err(SmfError::Invalid("missing MThd header"));
    }
    let header_len = u32::from_be_bytes(
        r.take(4)?
            .try_into()
            .map_err(|_| SmfError::Invalid("header"))?,
    ) as usize;
    let header = r.take(header_len)?;
    if header.len() < 6 {
        return Err(SmfError::Invalid("short header"));
    }
    let tracks_declared = u16::from_be_bytes([header[2], header[3]]);
    let division = u16::from_be_bytes([header[4], header[5]]);
    if division & 0x8000 != 0 {
        return Err(SmfError::Smpte);
    }
    let mut smf = Smf {
        ppq: division.max(1),
        tempo_us: 500_000,
        tracks: Vec::new(),
    };
    let mut tempo_found = false;
    while r.pos < data.len() && smf.tracks.len() <= usize::from(tracks_declared) {
        let id = r.take(4)?;
        let id: [u8; 4] = id.try_into().map_err(|_| SmfError::Invalid("chunk"))?;
        let len = u32::from_be_bytes(
            r.take(4)?
                .try_into()
                .map_err(|_| SmfError::Invalid("chunk"))?,
        ) as usize;
        let chunk = r.take(len)?;
        if &id != b"MTrk" {
            continue;
        }
        let mut t = Reader {
            data: chunk,
            pos: 0,
        };
        let mut track = SmfTrack::default();
        let mut tick = 0u64;
        let mut running = 0u8;
        let mut open: Vec<(u8, u8, u64, u8)> = Vec::new();
        while t.pos < chunk.len() {
            tick = tick.saturating_add(t.vlq()?);
            let mut status = t.byte()?;
            if status < 0x80 {
                // Running status: the byte read is the first data byte.
                if running == 0 {
                    return Err(SmfError::Invalid("running status without a status byte"));
                }
                t.pos -= 1;
                status = running;
            }
            match status {
                0xFF => {
                    let kind = t.byte()?;
                    let len = t.vlq()? as usize;
                    let body = t.take(len)?;
                    match kind {
                        0x03 => track.name = String::from_utf8_lossy(body).into_owned(),
                        0x51 if body.len() == 3 && !tempo_found => {
                            smf.tempo_us = u32::from_be_bytes([0, body[0], body[1], body[2]]);
                            tempo_found = true;
                        }
                        0x2F => break,
                        _ => {}
                    }
                }
                0xF0 | 0xF7 => {
                    let len = t.vlq()? as usize;
                    t.take(len)?;
                }
                _ => {
                    running = status;
                    let kind = status & 0xF0;
                    let channel = status & 0x0F;
                    let a = t.byte()?;
                    let b = if matches!(kind, 0xC0 | 0xD0) {
                        0
                    } else {
                        t.byte()?
                    };
                    let on = kind == 0x90 && b > 0;
                    let off = kind == 0x80 || (kind == 0x90 && b == 0);
                    if on {
                        open.push((a, channel, tick, b));
                    } else if off
                        && let Some(i) = open.iter().position(|(k, c, ..)| *k == a && *c == channel)
                    {
                        let (key, channel, start, velocity) = open.remove(i);
                        track.notes.push(SmfNote {
                            start,
                            length: tick - start,
                            key,
                            velocity,
                            channel,
                        });
                    }
                }
            }
        }
        // Notes never switched off end where the track ends.
        for (key, channel, start, velocity) in open {
            track.notes.push(SmfNote {
                start,
                length: tick.saturating_sub(start),
                key,
                velocity,
                channel,
            });
        }
        track.notes.sort_by_key(|n| (n.start, n.key));
        if !track.notes.is_empty() {
            smf.tracks.push(track);
        }
    }
    Ok(smf)
}

pub fn write(path: &Path, smf: &Smf) -> Result<(), SmfError> {
    fs::write(path, to_bytes(smf))?;
    Ok(())
}

pub fn read(path: &Path) -> Result<Smf, SmfError> {
    if fs::metadata(path)?.len() > MAX_SMF_BYTES {
        return Err(SmfError::TooLarge);
    }
    from_bytes(&fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Smf {
        Smf {
            ppq: 96,
            tempo_us: 461_538,
            tracks: vec![SmfTrack {
                name: "Lead".into(),
                notes: vec![
                    SmfNote {
                        start: 0,
                        length: 96,
                        key: 60,
                        velocity: 100,
                        channel: 0,
                    },
                    SmfNote {
                        start: 96,
                        length: 48,
                        key: 64,
                        velocity: 90,
                        channel: 0,
                    },
                    SmfNote {
                        start: 96,
                        length: 96,
                        key: 67,
                        velocity: 80,
                        channel: 0,
                    },
                ],
            }],
        }
    }

    #[test]
    fn round_trips_notes_tempo_and_names() {
        let smf = sample();
        let parsed = from_bytes(&to_bytes(&smf)).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(parsed, smf);
    }

    #[test]
    fn running_status_and_zero_velocity_note_off_are_understood() {
        // Header (type 0, 1 track, 480 PPQ) + a track using running status.
        let mut data = b"MThd\0\0\0\x06\0\0\0\x01\x01\xE0".to_vec();
        let track = [
            0x00, 0x90, 60, 100, // note on
            0x60, 62, 100, // running status: another note on, delta 96
            0x60, 60, 0, // running status: velocity 0 = note off
            0x00, 62, 0, 0x00, 0xFF, 0x2F, 0x00,
        ];
        data.extend_from_slice(b"MTrk");
        data.extend_from_slice(&(track.len() as u32).to_be_bytes());
        data.extend_from_slice(&track);
        let smf = from_bytes(&data).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(smf.ppq, 480);
        let notes = &smf.tracks[0].notes;
        assert_eq!(notes.len(), 2);
        assert_eq!(
            (notes[0].key, notes[0].start, notes[0].length),
            (60, 0, 192)
        );
        assert_eq!(
            (notes[1].key, notes[1].start, notes[1].length),
            (62, 96, 96)
        );
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(from_bytes(b"RIFF....").is_err());
        assert!(
            from_bytes(b"MThd\0\0\0\x06\0\x01\0\x01\x80\x00").is_err(),
            "SMPTE"
        );
        assert!(from_bytes(b"MThd\0\0").is_err());
    }
}
