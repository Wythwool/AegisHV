use std::fs::File;
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use crate::vmi::{
    GuestMemoryReader, GuestPhysical, MemoryReadError, SyntheticGuestPhysicalMemoryReader, VmId,
};

pub const OFFLINE_MEMORY_SNAPSHOT_FORMAT: &str = "aegishv-memory-map-v1";

#[derive(Debug, Clone)]
pub struct OfflineGuestMemorySnapshotReader {
    inner: SyntheticGuestPhysicalMemoryReader,
}

impl OfflineGuestMemorySnapshotReader {
    pub fn from_manifest(path: impl AsRef<Path>) -> Result<Self, MemoryReadError> {
        let path = path.as_ref();
        let manifest = std::fs::read_to_string(path).map_err(|err| {
            MemoryReadError::TemporarilyUnavailable {
                resource: "snapshot-manifest",
                detail: format!("cannot read offline memory snapshot manifest: {err}"),
            }
        })?;
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        Self::from_manifest_text(&manifest, base_dir)
    }

    pub fn from_manifest_text(
        manifest: &str,
        base_dir: impl AsRef<Path>,
    ) -> Result<Self, MemoryReadError> {
        let base_dir = base_dir.as_ref();
        let mut reader = SyntheticGuestPhysicalMemoryReader::new();
        let mut saw_version = false;

        for (idx, raw_line) in manifest.lines().enumerate() {
            let line_no = idx + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if !saw_version {
                if line != OFFLINE_MEMORY_SNAPSHOT_FORMAT {
                    return Err(malformed(
                        line_no,
                        format!("expected {OFFLINE_MEMORY_SNAPSHOT_FORMAT}, found '{line}'"),
                    ));
                }
                saw_version = true;
                continue;
            }

            parse_manifest_entry(line_no, line, base_dir, &mut reader)?;
        }

        if !saw_version {
            return Err(malformed(
                1,
                format!("missing {OFFLINE_MEMORY_SNAPSHOT_FORMAT} header"),
            ));
        }

        Ok(Self { inner: reader })
    }
}

impl GuestMemoryReader for OfflineGuestMemorySnapshotReader {
    fn read_physical(
        &self,
        vm: VmId,
        gpa: GuestPhysical,
        buf: &mut [u8],
    ) -> Result<usize, MemoryReadError> {
        self.inner.read_physical(vm, gpa, buf)
    }
}

fn parse_manifest_entry(
    line_no: usize,
    line: &str,
    base_dir: &Path,
    reader: &mut SyntheticGuestPhysicalMemoryReader,
) -> Result<(), MemoryReadError> {
    let mut fields = line.split_whitespace();
    let kind = required_field(line_no, fields.next(), "entry kind")?;

    match kind {
        "map" => {
            let gpa = parse_gpa(line_no, required_field(line_no, fields.next(), "gpa")?)?;
            let len = parse_len(line_no, required_field(line_no, fields.next(), "len")?)?;
            let rel_path = parse_backing_path(
                line_no,
                required_field(line_no, fields.next(), "backing path")?,
            )?;
            let offset = parse_u64(
                line_no,
                "file offset",
                required_field(line_no, fields.next(), "file offset")?,
            )?;
            reject_extra_fields(line_no, fields.next())?;

            match load_backing_bytes(line_no, base_dir, &rel_path, offset, len)? {
                BackingBytes::Loaded(bytes) => reader.map_range(gpa, bytes),
                BackingBytes::Unavailable(detail) => {
                    reader.mark_unavailable_range(gpa, len, detail)
                }
            }
        }
        "bytes" => {
            let gpa = parse_gpa(line_no, required_field(line_no, fields.next(), "gpa")?)?;
            let bytes = parse_hex_bytes(
                line_no,
                required_field(line_no, fields.next(), "hex bytes")?,
            )?;
            reject_extra_fields(line_no, fields.next())?;
            reader.map_range(gpa, bytes)
        }
        "deny" => {
            let gpa = parse_gpa(line_no, required_field(line_no, fields.next(), "gpa")?)?;
            let len = parse_len(line_no, required_field(line_no, fields.next(), "len")?)?;
            let detail = remaining_detail(line_no, fields)?;
            reader.deny_range(gpa, len, detail)
        }
        "unavailable" => {
            let gpa = parse_gpa(line_no, required_field(line_no, fields.next(), "gpa")?)?;
            let len = parse_len(line_no, required_field(line_no, fields.next(), "len")?)?;
            let detail = remaining_detail(line_no, fields)?;
            reader.mark_unavailable_range(gpa, len, detail)
        }
        _ => Err(malformed(
            line_no,
            format!("unsupported entry kind '{kind}'"),
        )),
    }
}

enum BackingBytes {
    Loaded(Vec<u8>),
    Unavailable(String),
}

fn load_backing_bytes(
    line_no: usize,
    base_dir: &Path,
    rel_path: &Path,
    offset: u64,
    len: usize,
) -> Result<BackingBytes, MemoryReadError> {
    let mut file = match File::open(base_dir.join(rel_path)) {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Ok(BackingBytes::Unavailable(format!(
                "backing file '{}' is missing",
                rel_path.display()
            )));
        }
        Err(err) => {
            return Ok(BackingBytes::Unavailable(format!(
                "cannot open backing file '{}': {err}",
                rel_path.display()
            )));
        }
    };

    file.seek(SeekFrom::Start(offset)).map_err(|err| {
        malformed(
            line_no,
            format!(
                "cannot seek backing file '{}' to offset {offset}: {err}",
                rel_path.display()
            ),
        )
    })?;

    let mut bytes = vec![0u8; len];
    match file.read_exact(&mut bytes) {
        Ok(()) => Ok(BackingBytes::Loaded(bytes)),
        Err(err) if err.kind() == ErrorKind::UnexpectedEof => {
            Ok(BackingBytes::Unavailable(format!(
                "backing file '{}' does not contain {len} bytes at offset {offset}",
                rel_path.display()
            )))
        }
        Err(err) => Ok(BackingBytes::Unavailable(format!(
            "cannot read backing file '{}': {err}",
            rel_path.display()
        ))),
    }
}

fn required_field<'a>(
    line_no: usize,
    field: Option<&'a str>,
    name: &str,
) -> Result<&'a str, MemoryReadError> {
    field.ok_or_else(|| malformed(line_no, format!("missing {name}")))
}

fn reject_extra_fields(line_no: usize, field: Option<&str>) -> Result<(), MemoryReadError> {
    match field {
        Some(value) => Err(malformed(line_no, format!("unexpected field '{value}'"))),
        None => Ok(()),
    }
}

fn remaining_detail<'a>(
    line_no: usize,
    fields: impl Iterator<Item = &'a str>,
) -> Result<String, MemoryReadError> {
    let detail = fields.collect::<Vec<_>>().join(" ");
    if detail.is_empty() {
        Err(malformed(line_no, "missing range detail"))
    } else {
        Ok(detail)
    }
}

fn parse_gpa(line_no: usize, token: &str) -> Result<GuestPhysical, MemoryReadError> {
    parse_u64(line_no, "gpa", token).map(GuestPhysical)
}

fn parse_len(line_no: usize, token: &str) -> Result<usize, MemoryReadError> {
    let len = parse_u64(line_no, "len", token)?;
    usize::try_from(len).map_err(|_| malformed(line_no, "length does not fit this target"))
}

fn parse_hex_bytes(line_no: usize, token: &str) -> Result<Vec<u8>, MemoryReadError> {
    let hex = token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))
        .unwrap_or(token);
    if hex.is_empty() || hex.len() % 2 != 0 {
        return Err(malformed(line_no, "hex bytes must contain full bytes"));
    }

    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair)
                .map_err(|_| malformed(line_no, "hex bytes must be ASCII"))?;
            u8::from_str_radix(text, 16)
                .map_err(|_| malformed(line_no, format!("invalid hex byte '{text}'")))
        })
        .collect()
}

fn parse_u64(line_no: usize, field: &str, token: &str) -> Result<u64, MemoryReadError> {
    let parsed = if let Some(hex) = token.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
    } else {
        token.parse::<u64>()
    };

    parsed.map_err(|_| malformed(line_no, format!("invalid {field} value '{token}'")))
}

fn parse_backing_path(line_no: usize, token: &str) -> Result<PathBuf, MemoryReadError> {
    let path = Path::new(token);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(MemoryReadError::PermissionDenied {
            operation: "load_snapshot_backing_file",
            detail: format!("line {line_no}: backing path must be relative"),
        });
    }

    if !path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(MemoryReadError::PermissionDenied {
            operation: "load_snapshot_backing_file",
            detail: format!("line {line_no}: backing path must stay under the manifest directory"),
        });
    }

    Ok(path.to_path_buf())
}

fn malformed(line_no: usize, detail: impl Into<String>) -> MemoryReadError {
    MemoryReadError::Malformed {
        detail: format!("line {line_no}: {}", detail.into()),
    }
}
