//! Motion JPEG in an AVI file: every frame is a JPEG picture. Any player opens
//! it and no codec has to be licensed.
//!
//! Frames are written as they come. [`Writer::finish`] fills in the totals in
//! the header and adds the index, which is what makes the file complete.

use std::io::{self, Seek, SeekFrom, Write};

/// The most bytes one file takes before the caller should start another;
/// AVI cannot go past 4 GiB and some players stop earlier.
pub const MAX_BYTES: u64 = 1 << 30;

/// Where the numbers that are only known at the end sit in the file.
const RIFF_SIZE_AT: u64 = 4;
const TOTAL_FRAMES_AT: u64 = 48;
const STREAM_LENGTH_AT: u64 = 140;
const MOVI_SIZE_AT: u64 = 216;
/// The first byte after the header, where the frames start: right after the `movi` list's size and name.
const MOVI_START: u64 = 224;

/// One entry of the index at the end.
struct Entry {
    keyframe: bool,
    offset: u32,
    size: u32,
}

/// Writes one recording.
pub struct Writer<W: Write + Seek> {
    output: W,
    entries: Vec<Entry>,
    /// Bytes of frame data written so far, with their chunk headers.
    movi_bytes: u64,
}

fn le32(output: &mut impl Write, value: u32) -> io::Result<()> {
    output.write_all(&value.to_le_bytes())
}

fn le16(output: &mut impl Write, value: u16) -> io::Result<()> {
    output.write_all(&value.to_le_bytes())
}

impl<W: Write + Seek> Writer<W> {
    /// Starts a file of pictures `width` by `height` at `frames_per_second`.
    pub fn start(
        mut output: W,
        width: u32,
        height: u32,
        frames_per_second: u32,
    ) -> io::Result<Self> {
        let rate = frames_per_second.max(1);
        let image_bytes = width.saturating_mul(height).saturating_mul(3);
        output.write_all(b"RIFF")?;
        le32(&mut output, 0)?; // size, filled in at the end
        output.write_all(b"AVI ")?;

        // The list of headers: its name, the avih chunk, and the stream list with its own header.
        output.write_all(b"LIST")?;
        le32(&mut output, 4 + (8 + 56) + (8 + 4 + (8 + 56) + (8 + 40)))?;
        output.write_all(b"hdrl")?;

        output.write_all(b"avih")?;
        le32(&mut output, 56)?;
        le32(&mut output, 1_000_000 / rate)?; // microseconds per frame
        le32(&mut output, 0)?; // maximum bytes per second
        le32(&mut output, 0)?; // padding
        le32(&mut output, 0x10)?; // has an index
        le32(&mut output, 0)?; // total frames, filled in at the end
        le32(&mut output, 0)?; // initial frames
        le32(&mut output, 1)?; // streams
        le32(&mut output, image_bytes)?; // suggested buffer size
        le32(&mut output, width)?;
        le32(&mut output, height)?;
        for _ in 0..4 {
            le32(&mut output, 0)?; // reserved
        }

        output.write_all(b"LIST")?;
        le32(&mut output, 4 + (8 + 56) + (8 + 40))?;
        output.write_all(b"strl")?;

        output.write_all(b"strh")?;
        le32(&mut output, 56)?;
        output.write_all(b"vids")?;
        output.write_all(b"MJPG")?;
        le32(&mut output, 0)?; // flags
        le16(&mut output, 0)?; // priority
        le16(&mut output, 0)?; // language
        le32(&mut output, 0)?; // initial frames
        le32(&mut output, 1)?; // scale
        le32(&mut output, rate)?;
        le32(&mut output, 0)?; // start
        le32(&mut output, 0)?; // length in frames, filled in at the end
        le32(&mut output, image_bytes)?;
        le32(&mut output, u32::MAX)?; // quality: default
        le32(&mut output, 0)?; // sample size: varies
        le16(&mut output, 0)?;
        le16(&mut output, 0)?;
        le16(&mut output, u16::try_from(width).unwrap_or(u16::MAX))?;
        le16(&mut output, u16::try_from(height).unwrap_or(u16::MAX))?;

        output.write_all(b"strf")?;
        le32(&mut output, 40)?;
        le32(&mut output, 40)?;
        le32(&mut output, width)?;
        le32(&mut output, height)?;
        le16(&mut output, 1)?; // planes
        le16(&mut output, 24)?; // bits per pixel
        output.write_all(b"MJPG")?;
        le32(&mut output, image_bytes)?;
        for _ in 0..4 {
            le32(&mut output, 0)?;
        }

        output.write_all(b"LIST")?;
        le32(&mut output, 0)?; // size of the frames, filled in at the end
        output.write_all(b"movi")?;
        Ok(Self {
            output,
            entries: Vec::new(),
            movi_bytes: 0,
        })
    }

    /// Adds a frame, a complete JPEG file.
    pub fn add_frame(&mut self, jpeg: &[u8]) -> io::Result<()> {
        self.add(jpeg, true)
    }

    /// Adds a frame that is the same as the one before: the player keeps showing it.
    pub fn add_repeat(&mut self) -> io::Result<()> {
        self.add(&[], false)
    }

    fn add(&mut self, data: &[u8], keyframe: bool) -> io::Result<()> {
        let size = u32::try_from(data.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a frame is too big"))?;
        // Offsets in the index count from the `movi` name, 4 bytes before the first chunk.
        let offset = u32::try_from(self.movi_bytes + 4)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the file is too big"))?;
        self.output.write_all(b"00dc")?;
        le32(&mut self.output, size)?;
        self.output.write_all(data)?;
        let padding = data.len() % 2;
        if padding == 1 {
            self.output.write_all(&[0])?;
        }
        self.movi_bytes += 8 + u64::from(size) + padding as u64;
        self.entries.push(Entry {
            keyframe,
            offset,
            size,
        });
        Ok(())
    }

    /// How many frames, repeats included, have been added.
    pub fn frames(&self) -> u32 {
        self.entries.len() as u32
    }

    /// The size the file has now.
    pub fn bytes(&self) -> u64 {
        MOVI_START + self.movi_bytes
    }

    /// Completes the file: the index at the end and the totals in the header.
    pub fn finish(mut self) -> io::Result<W> {
        let index_bytes = self.entries.len() as u64 * 16;
        self.output.write_all(b"idx1")?;
        le32(&mut self.output, index_bytes as u32)?;
        for entry in &self.entries {
            self.output.write_all(b"00dc")?;
            le32(&mut self.output, if entry.keyframe { 0x10 } else { 0 })?;
            le32(&mut self.output, entry.offset)?;
            le32(&mut self.output, entry.size)?;
        }
        let end = MOVI_START + self.movi_bytes + 8 + index_bytes;
        self.patch(RIFF_SIZE_AT, (end - 8) as u32)?;
        self.patch(TOTAL_FRAMES_AT, self.entries.len() as u32)?;
        self.patch(STREAM_LENGTH_AT, self.entries.len() as u32)?;
        self.patch(MOVI_SIZE_AT, (self.movi_bytes + 4) as u32)?;
        self.output.seek(SeekFrom::End(0))?;
        self.output.flush()?;
        Ok(self.output)
    }

    fn patch(&mut self, at: u64, value: u32) -> io::Result<()> {
        self.output.seek(SeekFrom::Start(at))?;
        le32(&mut self.output, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn word(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    fn file(frames: &[Option<&[u8]>]) -> Vec<u8> {
        let mut writer = Writer::start(Cursor::new(Vec::new()), 64, 48, 10).unwrap();
        for frame in frames {
            match frame {
                Some(jpeg) => writer.add_frame(jpeg).unwrap(),
                None => writer.add_repeat().unwrap(),
            }
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn the_file_has_the_riff_layout_and_sizes_that_add_up() {
        let bytes = file(&[Some(b"abc"), None, Some(b"defgh")]);

        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"AVI ");
        assert_eq!(word(&bytes, 4) as usize, bytes.len() - 8);
        assert_eq!(&bytes[12..16], b"LIST");
        assert_eq!(&bytes[20..24], b"hdrl");
    }

    #[test]
    fn the_header_counts_the_frames_and_names_the_size() {
        let bytes = file(&[Some(b"abc"), None, Some(b"defgh")]);

        // avih: microseconds per frame, then the totals.
        assert_eq!(word(&bytes, 32), 100_000);
        assert_eq!(word(&bytes, TOTAL_FRAMES_AT as usize), 3);
        assert_eq!(word(&bytes, 64), 64); // width
        assert_eq!(word(&bytes, 68), 48); // height
        assert_eq!(word(&bytes, STREAM_LENGTH_AT as usize), 3);
        assert_eq!(&bytes[108..112], b"vids");
        assert_eq!(&bytes[112..116], b"MJPG");
    }

    #[test]
    fn the_header_list_is_exactly_as_long_as_it_says() {
        let bytes = file(&[Some(b"abc")]);

        // The list starts at byte 12 with its name and size, and the `movi` list follows its content.
        let header_list = word(&bytes, 16) as usize;
        assert_eq!(
            &bytes[12 + 8 + header_list..12 + 8 + header_list + 4],
            b"LIST"
        );
        let stream_list = word(&bytes, 92) as usize;
        assert_eq!(
            &bytes[88 + 8 + stream_list..88 + 8 + stream_list + 4],
            b"LIST"
        );
    }

    #[test]
    fn frames_sit_in_the_movi_list_padded_to_even_sizes() {
        let bytes = file(&[Some(b"abc"), Some(b"de")]);

        assert_eq!(&bytes[220..224], b"movi");
        assert_eq!(&bytes[224..228], b"00dc");
        assert_eq!(word(&bytes, 228), 3);
        assert_eq!(&bytes[232..235], b"abc");
        // One byte of padding after an odd frame; the next chunk starts on an even byte.
        assert_eq!(&bytes[236..240], b"00dc");
        // The list's size covers its name and both chunks: 4 + (8 + 4) + (8 + 2).
        assert_eq!(word(&bytes, MOVI_SIZE_AT as usize), 26);
    }

    #[test]
    fn the_index_lists_every_frame_with_its_offset_and_size() {
        let bytes = file(&[Some(b"abc"), None]);

        let idx = bytes.windows(4).position(|w| w == b"idx1").unwrap();
        assert_eq!(word(&bytes, idx + 4), 32);
        // First entry: keyframe at offset 4 (right after the list name), 3 bytes.
        assert_eq!(&bytes[idx + 8..idx + 12], b"00dc");
        assert_eq!(word(&bytes, idx + 12), 0x10);
        assert_eq!(word(&bytes, idx + 16), 4);
        assert_eq!(word(&bytes, idx + 20), 3);
        // Second: a repeat, no keyframe, empty, after the first chunk and its padding.
        assert_eq!(word(&bytes, idx + 28), 0);
        assert_eq!(word(&bytes, idx + 32), 4 + 8 + 4);
        assert_eq!(word(&bytes, idx + 36), 0);
    }

    #[test]
    fn the_writer_knows_how_many_frames_and_bytes_it_holds() {
        let mut writer = Writer::start(Cursor::new(Vec::new()), 64, 48, 10).unwrap();
        let before = writer.bytes();

        writer.add_frame(b"abc").unwrap();
        writer.add_repeat().unwrap();

        assert_eq!(writer.frames(), 2);
        assert_eq!(writer.bytes(), before + (8 + 4) + 8);
    }
}
