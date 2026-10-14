//! FMOD sound banks (`FSB5`), the payload of every Unity `AudioClip`
//! (`rainworld-re/notes/sound.md` section 3): one sample per bank, 16-bit PCM or FMOD's
//! headerless Vorbis.
//!
//! FMOD Vorbis stores only the audio packets (each behind a 16-bit length) and the CRC32
//! of the Vorbis setup header; the setup headers themselves are a table inside FMOD. The
//! port ships the ones the game uses ([`VorbisSetups`], from vgmstream's
//! `vorbis_codebooks_fsb.h`; provenance in `data/README.md`) and rebuilds the
//! identification and comment headers, then decodes with `lewton`.

use lewton::audio::{PreviousWindowRight, read_audio_packet};
use lewton::header::{IdentHeader, SetupHeader, read_header_ident, read_header_setup};

use super::reader::Reader;

/// Decoded audio: interleaved signed 16-bit samples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcmAudio {
    pub channels: u16,
    pub sample_rate: u32,
    pub samples: Vec<i16>,
}

impl PcmAudio {
    /// Samples per channel.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels.max(1))
    }
}

/// Sample encoding of a bank (`FMOD_SOUND_FORMAT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fsb5Mode {
    Pcm16,
    Vorbis,
    Other(u32),
}

/// A parsed one-sample FSB5 bank, borrowing its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fsb5<'a> {
    pub mode: Fsb5Mode,
    pub channels: u16,
    pub sample_rate: u32,
    /// Samples per channel.
    pub frames: u32,
    /// CRC32 of the Vorbis setup header (chunk type 11).
    pub vorbis_crc: Option<u32>,
    data: &'a [u8],
}

/// The FMOD Vorbis setup headers the port ships, keyed by their CRC32.
#[derive(Debug, Clone, Copy)]
pub struct VorbisSetups;

impl VorbisSetups {
    /// `u32` count, then per entry `u32` CRC, `u32` length and the setup packet
    /// (little-endian).
    const TABLE: &'static [u8] = include_bytes!("../../data/fmod_vorbis_setups.bin");

    /// The setup packet with this CRC.
    #[must_use]
    pub fn get(crc: u32) -> Option<&'static [u8]> {
        Self::entries().find(|(c, _)| *c == crc).map(|(_, p)| p)
    }

    /// Every shipped CRC and packet.
    pub fn entries() -> impl Iterator<Item = (u32, &'static [u8])> {
        let count = Self::word(0) as usize;
        let mut at = 4;
        (0..count).map(move |_| {
            let (crc, len) = (Self::word(at), Self::word(at + 4) as usize);
            let packet = &Self::TABLE[at + 8..at + 8 + len];
            at += 8 + len;
            (crc, packet)
        })
    }

    fn word(at: usize) -> u32 {
        u32::from_le_bytes(Self::TABLE[at..at + 4].try_into().unwrap())
    }
}

impl<'a> Fsb5<'a> {
    const BASE_HEADER: usize = 60;
    const VORBIS_CHUNK: u32 = 11;
    const CHANNELS_CHUNK: u32 = 1;
    const FREQUENCY_CHUNK: u32 = 2;
    /// FMOD Vorbis block sizes: short 256, long 2048 (as exponents).
    const BLOCKSIZES: u8 = 8 | (11 << 4);

    /// Parses the header and the first sample's header and chunks.
    ///
    /// # Errors
    /// When the bytes are not a one-sample FSB5 version 1 bank.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        let mut r = Reader::new(bytes);
        if r.take(4)? != b"FSB5" {
            return Err("not an FSB5 bank".into());
        }
        let version = r.u32()?;
        let samples = r.u32()?;
        let sample_headers = r.u32()? as usize;
        let names = r.u32()? as usize;
        let data_size = r.u32()? as usize;
        let mode = match r.u32()? {
            2 => Fsb5Mode::Pcm16,
            15 => Fsb5Mode::Vorbis,
            other => Fsb5Mode::Other(other),
        };
        if version != 1 || samples != 1 {
            return Err(format!("FSB5 version {version} with {samples} samples"));
        }
        r.seek(Self::BASE_HEADER);
        let header = r.u64()?;
        let mut more = header & 1 != 0;
        let mut sample_rate = Self::frequency((header >> 1) & 0xF)?;
        let mut channels = if header >> 5 & 1 != 0 { 2 } else { 1 };
        let offset = ((header >> 6) & 0x0FFF_FFFF) as usize * 32;
        let frames = (header >> 34) as u32;
        let mut vorbis_crc = None;
        while more {
            let chunk = r.u32()?;
            more = chunk & 1 != 0;
            let size = ((chunk >> 1) & 0xFF_FFFF) as usize;
            let kind = chunk >> 25;
            let body = r.take(size)?;
            let mut c = Reader::new(body);
            match kind {
                Self::CHANNELS_CHUNK => channels = u16::from(c.u8()?),
                Self::FREQUENCY_CHUNK => sample_rate = c.u32()?,
                Self::VORBIS_CHUNK => vorbis_crc = Some(c.u32()?),
                _ => {}
            }
        }
        let start = Self::BASE_HEADER + sample_headers + names + offset;
        let data = bytes
            .get(start..Self::BASE_HEADER + sample_headers + names + data_size)
            .ok_or("FSB5 data runs past the bank")?;
        Ok(Self {
            mode,
            channels,
            sample_rate,
            frames,
            vorbis_crc,
            data,
        })
    }

    /// Sample rate of a frequency index in the sample header.
    fn frequency(index: u64) -> Result<u32, String> {
        Ok(match index {
            1 => 8000,
            2 => 11_000,
            3 => 11_025,
            4 => 16_000,
            5 => 22_050,
            6 => 24_000,
            7 => 32_000,
            8 => 44_100,
            9 => 48_000,
            other => return Err(format!("FSB5 frequency index {other}")),
        })
    }

    /// Decodes the whole sample to `frames` frames.
    ///
    /// # Errors
    /// For other encodings, a missing Vorbis setup header or corrupt data.
    pub fn decode(&self) -> Result<PcmAudio, String> {
        let samples = match self.mode {
            Fsb5Mode::Pcm16 => self.decode_pcm16()?,
            Fsb5Mode::Vorbis => self.decode_vorbis()?,
            Fsb5Mode::Other(m) => return Err(format!("FSB5 mode {m} is not decoded")),
        };
        Ok(PcmAudio {
            channels: self.channels,
            sample_rate: self.sample_rate,
            samples,
        })
    }

    /// The stored region is padded; only `frames * channels` samples are audio.
    fn decode_pcm16(&self) -> Result<Vec<i16>, String> {
        let len = self.frames as usize * usize::from(self.channels) * 2;
        let bytes = self
            .data
            .get(..len)
            .ok_or("PCM16 data shorter than its sample count")?;
        Ok(bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes(*b))
            .collect())
    }

    /// Rebuilds the three Vorbis headers and decodes the length-prefixed packets; the
    /// output is cut to `frames` (FMOD keeps no granule positions).
    fn decode_vorbis(&self) -> Result<Vec<i16>, String> {
        let crc = self.vorbis_crc.ok_or("FSB5 Vorbis without a setup CRC")?;
        let setup_packet =
            VorbisSetups::get(crc).ok_or_else(|| format!("no Vorbis setup header {crc:08x}"))?;
        let ident = read_header_ident(&self.ident_packet()).map_err(|e| format!("{e:?}"))?;
        let setup: SetupHeader = read_header_setup(
            setup_packet,
            ident.audio_channels,
            (ident.blocksize_0, ident.blocksize_1),
        )
        .map_err(|e| format!("setup {crc:08x}: {e:?}"))?;
        let channels = usize::from(self.channels);
        let wanted = self.frames as usize * channels;
        let mut out = Vec::with_capacity(wanted);
        let mut pwr = PreviousWindowRight::new();
        let mut r = Reader::new(self.data);
        while out.len() < wanted && r.pos() + 2 <= self.data.len() {
            let len = usize::from(r.u16()?);
            if len == 0 || len == 0xFFFF {
                break;
            }
            let packet = r.take(len)?;
            let decoded: Vec<Vec<i16>> = read_audio_packet(&ident, &setup, packet, &mut pwr)
                .map_err(|e| format!("Vorbis packet at {}: {e:?}", r.pos()))?;
            Self::interleave(&decoded, &mut out);
        }
        if out.len() < wanted {
            return Err(format!(
                "Vorbis decoded {} of {} frames",
                out.len() / channels.max(1),
                self.frames
            ));
        }
        out.truncate(wanted);
        Ok(out)
    }

    fn interleave(channels: &[Vec<i16>], out: &mut Vec<i16>) {
        let frames = channels.first().map_or(0, Vec::len);
        for i in 0..frames {
            out.extend(channels.iter().map(|c| c[i]));
        }
    }

    /// The identification header FMOD leaves out: channels, rate, block sizes.
    fn ident_packet(&self) -> Vec<u8> {
        let mut p = vec![1];
        p.extend_from_slice(b"vorbis");
        p.extend_from_slice(&0u32.to_le_bytes());
        p.push(self.channels as u8);
        p.extend_from_slice(&self.sample_rate.to_le_bytes());
        p.extend_from_slice(&[0; 12]);
        p.push(Self::BLOCKSIZES);
        p.push(1);
        p
    }

    /// The identification header as `lewton` parses it.
    ///
    /// # Errors
    /// Never for a parsed bank; kept fallible as `lewton` is.
    pub fn ident_header(&self) -> Result<IdentHeader, String> {
        read_header_ident(&self.ident_packet()).map_err(|e| format!("{e:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-sample bank: header, sample header, optional chunk, data.
    fn bank(mode: u32, header: u64, chunk: Option<(u32, &[u8])>, data: &[u8]) -> Vec<u8> {
        let mut sample = header.to_le_bytes().to_vec();
        if let Some((kind, body)) = chunk {
            sample[0] |= 1;
            sample.extend_from_slice(&((kind << 25) | ((body.len() as u32) << 1)).to_le_bytes());
            sample.extend_from_slice(body);
        }
        let mut b = b"FSB5".to_vec();
        for v in [1, 1, sample.len() as u32, 0, data.len() as u32, mode] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.resize(60, 0);
        b.extend_from_slice(&sample);
        b.extend_from_slice(data);
        b
    }

    #[test]
    fn pcm16_header_and_padding() {
        // 44.1 kHz (index 8), stereo, 2 frames; 4 bytes of padding after the audio.
        let header = (8 << 1) | (1 << 5) | (2u64 << 34);
        let data = [1, 0, 2, 0, 3, 0, 0xFF, 0xFF, 9, 9, 9, 9];
        let bytes = bank(2, header, None, &data);
        let fsb = Fsb5::parse(&bytes).unwrap();
        assert_eq!(
            (fsb.mode, fsb.channels, fsb.sample_rate, fsb.frames),
            (Fsb5Mode::Pcm16, 2, 44_100, 2)
        );
        let pcm = fsb.decode().unwrap();
        assert_eq!(pcm.samples, [1, 2, 3, -1]);
        assert_eq!(pcm.frames(), 2);
    }

    #[test]
    fn vorbis_chunk_gives_the_crc() {
        let header = (8 << 1) | (5u64 << 34);
        let bytes = bank(15, header, Some((11, &0xDEAD_BEEFu32.to_le_bytes())), &[]);
        let fsb = Fsb5::parse(&bytes).unwrap();
        assert_eq!(fsb.vorbis_crc, Some(0xDEAD_BEEF));
        assert_eq!((fsb.channels, fsb.frames), (1, 5));
        assert!(fsb.decode().unwrap_err().contains("deadbeef"));
    }

    #[test]
    fn shipped_setups_are_vorbis_setup_packets() {
        let entries: Vec<_> = VorbisSetups::entries().collect();
        assert_eq!(entries.len(), 26);
        for (crc, packet) in entries {
            assert_eq!(&packet[..7], b"\x05vorbis", "{crc:08x}");
        }
    }
}
