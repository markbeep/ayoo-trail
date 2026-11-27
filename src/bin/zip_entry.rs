//! `trail-calc-zip-entry`: fetch one entry of a remote zip by HTTP Range (ADR 0007), so the
//! 434 MB `TLM_STRASSE.shp` comes out of the 3.6 GB swissTLM3D SHP zip without the rest.

use std::error::Error;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use flate2::read::DeflateDecoder;

#[derive(Parser)]
struct Args {
    /// Zip URL; the server must support Range requests.
    url: String,
    /// Entry name as stored in the zip (swisstopo uses `\` separators).
    entry: String,
    /// Output file.
    out: PathBuf,
}

type Res<T> = Result<T, Box<dyn Error>>;

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("trail-calc-zip-entry: {e}");
            ExitCode::FAILURE
        }
    }
}

fn range(url: &str, from: u64, len: u64) -> Res<Box<dyn Read + Send + Sync>> {
    let r = ureq::get(url)
        .set("Range", &format!("bytes={from}-{}", from + len - 1))
        .call()?;
    if r.status() != 206 {
        return Err(format!("{url}: no Range support (HTTP {})", r.status()).into());
    }
    Ok(r.into_reader())
}

fn run(a: Args) -> Res<()> {
    let size: u64 = ureq::head(&a.url)
        .call()?
        .header("Content-Length")
        .ok_or("no Content-Length")?
        .parse()?;
    let read_at = |from: u64, len: u64| -> Res<Vec<u8>> {
        let mut v = Vec::with_capacity(len as usize);
        range(&a.url, from, len)?.read_to_end(&mut v)?;
        Ok(v)
    };
    let e = locate(size, &read_at, &a.entry)?;
    let tmp = a.out.with_extension("part");
    let mut out = BufWriter::new(File::create(&tmp)?);
    let n = extract(range(&a.url, e.data, e.comp)?, e.method, &mut out)?;
    out.flush()?;
    if n != e.size {
        return Err(format!("{}: got {n} bytes, expected {}", a.entry, e.size).into());
    }
    std::fs::rename(&tmp, &a.out)?;
    Ok(())
}

#[derive(Debug, PartialEq)]
struct Entry {
    /// Offset of the compressed data.
    data: u64,
    comp: u64,
    size: u64,
    method: u16,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}

/// Find `name` through the (zip64) central directory; `read_at(offset, len)` reads the archive.
fn locate(size: u64, read_at: &dyn Fn(u64, u64) -> Res<Vec<u8>>, name: &str) -> Res<Entry> {
    // EOCD (22 bytes + comment ≤ 64 KiB), preceded by the 20-byte zip64 locator.
    let tail_len = size.min(22 + 65535 + 20);
    let tail = read_at(size - tail_len, tail_len)?;
    let eocd = (0..=tail.len() - 22)
        .rev()
        .find(|&i| u32_at(&tail, i) == 0x0605_4b50)
        .ok_or("no end of central directory")?;
    let (mut cd_len, mut cd_off) = (
        u32_at(&tail, eocd + 12) as u64,
        u32_at(&tail, eocd + 16) as u64,
    );
    if eocd >= 20 && u32_at(&tail, eocd - 20) == 0x0706_4b50 {
        let z = read_at(u64_at(&tail, eocd - 20 + 8), 56)?;
        if u32_at(&z, 0) != 0x0606_4b50 {
            return Err("bad zip64 end of central directory".into());
        }
        (cd_len, cd_off) = (u64_at(&z, 40), u64_at(&z, 48));
    }
    let cd = read_at(cd_off, cd_len)?;
    let mut i = 0;
    while i + 46 <= cd.len() && u32_at(&cd, i) == 0x0201_4b50 {
        let (nl, xl, cl) = (
            u16_at(&cd, i + 28) as usize,
            u16_at(&cd, i + 30) as usize,
            u16_at(&cd, i + 32) as usize,
        );
        let method = u16_at(&cd, i + 10);
        let mut comp = u32_at(&cd, i + 20) as u64;
        let mut size = u32_at(&cd, i + 24) as u64;
        let mut local = u32_at(&cd, i + 42) as u64;
        if &cd[i + 46..i + 46 + nl] == name.as_bytes() {
            // Zip64 extra field: present values, in order size, comp, local offset.
            let mut x = i + 46 + nl;
            let end = x + xl;
            while x + 4 <= end {
                let (id, len) = (u16_at(&cd, x), u16_at(&cd, x + 2) as usize);
                if id == 1 {
                    let mut p = x + 4;
                    for v in [&mut size, &mut comp, &mut local] {
                        if *v == 0xFFFF_FFFF {
                            *v = u64_at(&cd, p);
                            p += 8;
                        }
                    }
                }
                x += 4 + len;
            }
            let lh = read_at(local, 30)?;
            if u32_at(&lh, 0) != 0x0403_4b50 {
                return Err("bad local header".into());
            }
            let data = local + 30 + u16_at(&lh, 26) as u64 + u16_at(&lh, 28) as u64;
            return Ok(Entry {
                data,
                comp,
                size,
                method,
            });
        }
        i += 46 + nl + xl + cl;
    }
    Err(format!("{name}: not in the zip").into())
}

/// Decompress (deflate) or copy (stored) the entry's data into `out`; returns bytes written.
fn extract(data: impl Read, method: u16, out: &mut impl Write) -> Res<u64> {
    Ok(match method {
        0 => io::copy(&mut { data }, out)?,
        8 => io::copy(&mut DeflateDecoder::new(data), out)?,
        m => return Err(format!("compression method {m} not supported").into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::DeflateEncoder, Compression};

    /// A two-entry zip (stored, deflated) built by hand, with a comment after the EOCD.
    fn zip() -> (Vec<u8>, Vec<u8>) {
        let body: Vec<u8> = (0..5000u32).flat_map(|i| (i % 97).to_le_bytes()).collect();
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&body).unwrap();
        let comp = enc.finish().unwrap();
        let entries: [(&str, u16, &[u8], usize); 2] = [
            ("a.txt", 0, b"hello", 5),
            ("DIR\\b.shp", 8, &comp, body.len()),
        ];
        let (mut z, mut cd) = (Vec::new(), Vec::new());
        for (name, method, data, size) in entries {
            let off = z.len() as u32;
            let h = |v: &mut Vec<u8>, sig: u32| {
                v.extend(sig.to_le_bytes());
                v.extend([20, 0]);
                if sig == 0x0201_4b50 {
                    v.extend([20, 0]);
                }
                v.extend([0, 0]);
                v.extend(method.to_le_bytes());
                v.extend([0; 8]); // time, date, crc (not checked)
                v.extend((data.len() as u32).to_le_bytes());
                v.extend((size as u32).to_le_bytes());
                v.extend((name.len() as u16).to_le_bytes());
                v.extend(if sig == 0x0201_4b50 { 0u16 } else { 3 }.to_le_bytes()); // extra
                if sig == 0x0201_4b50 {
                    v.extend([0; 6]); // comment len, disk, internal attrs
                    v.extend([0; 4]);
                    v.extend(off.to_le_bytes());
                }
                v.extend(name.as_bytes());
            };
            h(&mut z, 0x0403_4b50);
            z.extend([0xAA; 3]); // local extra field, skipped
            z.extend(data);
            h(&mut cd, 0x0201_4b50);
        }
        let cd_off = z.len() as u32;
        z.extend(&cd);
        z.extend(0x0605_4b50u32.to_le_bytes());
        z.extend([0, 0, 0, 0, 2, 0, 2, 0]);
        z.extend((cd.len() as u32).to_le_bytes());
        z.extend(cd_off.to_le_bytes());
        z.extend(4u16.to_le_bytes());
        z.extend(b"note");
        (z, body)
    }

    #[test]
    fn finds_and_extracts_an_entry() {
        let (z, body) = zip();
        let read_at = |o: u64, l: u64| Ok(z[o as usize..(o + l) as usize].to_vec());
        let e = locate(z.len() as u64, &read_at, "DIR\\b.shp").unwrap();
        assert_eq!((e.method, e.size), (8, body.len() as u64));
        let mut out = Vec::new();
        let n = extract(
            &z[e.data as usize..(e.data + e.comp) as usize],
            e.method,
            &mut out,
        )
        .unwrap();
        assert_eq!((n, out), (body.len() as u64, body));

        let e = locate(z.len() as u64, &read_at, "a.txt").unwrap();
        let mut out = Vec::new();
        extract(
            &z[e.data as usize..(e.data + e.comp) as usize],
            e.method,
            &mut out,
        )
        .unwrap();
        assert_eq!(out, b"hello");
        assert!(locate(z.len() as u64, &read_at, "nope").is_err());
    }
}
