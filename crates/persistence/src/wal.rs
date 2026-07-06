//! The append-only write-ahead log: length-prefixed binary frames, one file per
//! UTC day, rotated at midnight.
//!
//! Frame format: a little-endian `u32` payload length followed by that many
//! bytes of bincode-encoded [`WalRecord`]. This is simple, fast, and
//! self-synchronizing enough for replay: a torn final frame (from a crash
//! mid-write) is detected as a short read and the reader stops cleanly at the
//! last intact record.
//!
//! The writer is synchronous. The engine runs it on a dedicated thread fed by a
//! channel, so the hot path enqueues a record and never blocks on disk. The day
//! bucket is derived from the record timestamp with a small civil-calendar
//! calculation, avoiding a date-library dependency.

use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use core_types::{Nanos, SeqNo};

use crate::error::Result;
use crate::record::{WalPayload, WalRecord};

const NANOS_PER_DAY: u64 = 86_400_000_000_000;

/// Appends records to daily-rotated WAL files in a directory.
pub struct WalWriter {
    dir: PathBuf,
    writer: Option<BufWriter<File>>,
    current_day: i64,
    next_seq: u64,
}

impl WalWriter {
    /// Opens (creating if needed) a writer over `dir`.
    pub fn open(dir: impl Into<PathBuf>) -> Result<WalWriter> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(WalWriter {
            dir,
            writer: None,
            current_day: i64::MIN,
            next_seq: 0,
        })
    }

    /// Sets the next sequence number (used to continue numbering after a
    /// restart from the last logged sequence).
    #[must_use]
    pub fn with_start_seq(mut self, seq: u64) -> WalWriter {
        self.next_seq = seq;
        self
    }

    /// Appends a payload stamped with `ts`, returning the assigned sequence
    /// number. Rotates to a new file when `ts` crosses into a new UTC day.
    pub fn append(&mut self, ts: Nanos, payload: WalPayload) -> Result<SeqNo> {
        let day = day_index(ts);
        if self.writer.is_none() || day != self.current_day {
            self.rotate(day)?;
        }

        let seq = SeqNo(self.next_seq);
        self.next_seq += 1;
        let record = WalRecord { seq, ts, payload };
        let bytes = bincode::serialize(&record)?;

        let writer = self.writer.as_mut().expect("writer is set after rotate");
        let len = u32::try_from(bytes.len()).expect("a single WAL record fits in u32");
        writer.write_all(&len.to_le_bytes())?;
        writer.write_all(&bytes)?;
        Ok(seq)
    }

    /// Flushes buffered bytes to the OS.
    pub fn flush(&mut self) -> Result<()> {
        if let Some(writer) = self.writer.as_mut() {
            writer.flush()?;
        }
        Ok(())
    }

    /// The file path a record with timestamp `ts` would be written to.
    #[must_use]
    pub fn path_for(&self, ts: Nanos) -> PathBuf {
        self.dir.join(wal_filename(day_index(ts)))
    }

    fn rotate(&mut self, day: i64) -> Result<()> {
        if let Some(writer) = self.writer.as_mut() {
            writer.flush()?;
        }
        let path = self.dir.join(wal_filename(day));
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        self.writer = Some(BufWriter::new(file));
        self.current_day = day;
        Ok(())
    }
}

impl Drop for WalWriter {
    fn drop(&mut self) {
        // Best-effort flush; errors on drop cannot be surfaced.
        let _ = self.flush();
    }
}

/// Reads [`WalRecord`]s from a WAL stream, tolerating a torn final frame.
pub struct WalReader<R: Read> {
    reader: R,
}

impl WalReader<BufReader<File>> {
    /// Opens a reader over a single WAL file.
    pub fn open(path: impl AsRef<Path>) -> Result<WalReader<BufReader<File>>> {
        let file = File::open(path)?;
        Ok(WalReader {
            reader: BufReader::new(file),
        })
    }
}

impl<R: Read> WalReader<R> {
    /// Wraps an arbitrary reader.
    pub fn new(reader: R) -> WalReader<R> {
        WalReader { reader }
    }

    /// Reads the next record, or `Ok(None)` at a clean end of stream or a torn
    /// final frame.
    pub fn next_record(&mut self) -> Result<Option<WalRecord>> {
        let mut len_buf = [0u8; 4];
        if !read_exact_or_eof(&mut self.reader, &mut len_buf)? {
            return Ok(None);
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        if !read_exact_or_eof(&mut self.reader, &mut buf)? {
            // Torn final frame: stop cleanly at the last intact record.
            return Ok(None);
        }
        let record = bincode::deserialize::<WalRecord>(&buf)?;
        Ok(Some(record))
    }

    /// Collects all intact records into a vector.
    pub fn read_all(mut self) -> Result<Vec<WalRecord>> {
        let mut out = Vec::new();
        while let Some(record) = self.next_record()? {
            out.push(record);
        }
        Ok(out)
    }
}

/// Reads exactly `buf.len()` bytes. Returns `Ok(true)` on success, `Ok(false)`
/// if the stream ended at a frame boundary (clean EOF) or mid-frame (torn).
fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<bool> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => return Ok(false), // EOF (clean if filled == 0, torn otherwise)
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(true)
}

/// The UTC day bucket for a timestamp (days since the Unix epoch).
#[must_use]
pub fn day_index(ts: Nanos) -> i64 {
    (ts.get() / NANOS_PER_DAY) as i64
}

/// The WAL filename for a day index, `wal-YYYY-MM-DD.bin`.
#[must_use]
pub fn wal_filename(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("wal-{y:04}-{m:02}-{d:02}.bin")
}

/// Converts days since the Unix epoch to a `(year, month, day)` civil date,
/// using Howard Hinnant's algorithm. Avoids a calendar-library dependency.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::{BookStatus, BookUpdate, Decimal, MarketEvent, PriceLevel, Symbol, Trade};

    fn sym() -> Symbol {
        Symbol::from_parts("BTC", "USDT").unwrap()
    }

    fn trade_event(id: u64) -> WalPayload {
        WalPayload::Market(MarketEvent::Trade(Trade {
            symbol: sym(),
            trade_id: id,
            price: Decimal::from_str_exact("60000.5").unwrap(),
            qty: Decimal::from_str_exact("0.01").unwrap(),
            buyer_is_maker: id.is_multiple_of(2),
            ts: Nanos(id),
        }))
    }

    fn book_event(seq: u64) -> WalPayload {
        WalPayload::Book(BookUpdate {
            symbol: sym(),
            seq: SeqNo(seq),
            update_id: seq,
            status: BookStatus::Live,
            best_bid: Some(PriceLevel::new(
                Decimal::from_str_exact("60000").unwrap(),
                Decimal::from_int(1),
            )),
            best_ask: Some(PriceLevel::new(
                Decimal::from_str_exact("60001").unwrap(),
                Decimal::from_int(1),
            )),
            ts: Nanos(seq),
        })
    }

    #[test]
    fn civil_dates_are_correct() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(18_262), (2020, 1, 1));
        assert_eq!(wal_filename(0), "wal-1970-01-01.bin");
    }

    #[test]
    fn roundtrip_records_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let ts = Nanos::from_millis(1_700_000_000_000); // a fixed day
        let mut writer = WalWriter::open(dir.path()).unwrap();
        let s0 = writer.append(ts, trade_event(1)).unwrap();
        let s1 = writer.append(ts, book_event(2)).unwrap();
        let s2 = writer.append(ts, trade_event(3)).unwrap();
        writer.flush().unwrap();

        assert_eq!((s0, s1, s2), (SeqNo(0), SeqNo(1), SeqNo(2)));

        let path = writer.path_for(ts);
        let records = WalReader::open(&path).unwrap().read_all().unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].payload, trade_event(1));
        assert_eq!(records[1].payload, book_event(2));
        assert_eq!(records[2].payload, trade_event(3));
        // Sequence numbers are strictly increasing.
        assert!(records[0].seq < records[1].seq && records[1].seq < records[2].seq);
    }

    #[test]
    fn rotates_on_day_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let day1 = Nanos::from_millis(1_700_000_000_000);
        let day2 = Nanos(day1.get() + NANOS_PER_DAY); // next UTC day
        let mut writer = WalWriter::open(dir.path()).unwrap();
        writer.append(day1, trade_event(1)).unwrap();
        writer.append(day2, trade_event(2)).unwrap();
        writer.flush().unwrap();

        assert_ne!(writer.path_for(day1), writer.path_for(day2));
        assert!(writer.path_for(day1).exists());
        assert!(writer.path_for(day2).exists());
        // Each file holds exactly one record.
        assert_eq!(
            WalReader::open(writer.path_for(day1))
                .unwrap()
                .read_all()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            WalReader::open(writer.path_for(day2))
                .unwrap()
                .read_all()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn tolerates_a_torn_final_frame() {
        let dir = tempfile::tempdir().unwrap();
        let ts = Nanos::from_millis(1_700_000_000_000);
        let path = {
            let mut writer = WalWriter::open(dir.path()).unwrap();
            writer.append(ts, trade_event(1)).unwrap();
            writer.append(ts, trade_event(2)).unwrap();
            writer.flush().unwrap();
            writer.path_for(ts)
        };
        // Truncate a few bytes off the end to simulate a crash mid-write.
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, &bytes[..bytes.len() - 3]).unwrap();

        let records = WalReader::open(&path).unwrap().read_all().unwrap();
        // The first record survives; the torn second one is dropped cleanly.
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].payload, trade_event(1));
    }
}
