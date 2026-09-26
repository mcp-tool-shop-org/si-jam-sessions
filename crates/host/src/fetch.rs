//! `host fetch-piano`: the piano's samples, downloaded, verified and
//! unpacked into a per-user cache directory (or the one named).
//!
//! 1. **Download.** FreePats' FLAC edition of the Salamander Grand Piano V3
//!    ([`URL`], 741,757,374 bytes), through the system's `curl`, which
//!    Windows (10 and later), macOS and Linux ship: the host links no TLS
//!    stack. A download that stopped resumes. `--archive` takes a copy
//!    obtained some other way instead.
//! 2. **Verify.** The archive's size and its SHA-256 must be the pinned
//!    [`ARCHIVE_BYTES`] and [`ARCHIVE_SHA256`], measured on the first
//!    download, on every fetch. FreePats publishes no checksum of its own. A
//!    download that does not match is deleted, and nothing is unpacked.
//! 3. **Unpack.** The gzip stream is inflated (flate2, which checks the
//!    stream's CRC-32 at its end) and the tar read by [`unpack`], which writes
//!    regular files and directories under the archive's one top directory and
//!    refuses anything else: a link, a path that climbs out, an absolute path,
//!    a name outside the set's characters, a header whose checksum is wrong.
//! 4. **Mark.** [`MARKER`] is written last, naming the archive and its
//!    SHA-256. The piano opens only a directory whose marker names the pinned
//!    archive ([`verified`]), so a fetch that stopped partway is never played.
//!    The archive is then deleted unless `--keep-archive`.
//!
//! The samples are never committed: the cache is outside the repository, and
//! a `--dir` inside it would be the user's own choice.
//!
//! # Standards compliance
//!
//! The studio's six workflow standards, scored 0-3 for this pipeline.
//!
//! - **PIN_PER_STEP 2.** The URL, the size and the SHA-256 are constants of
//!   the host, so a changed archive is a changed host, reviewed.
//! - **ANDON_AUTHORITY 2.** Each step stops the fetch on a defect: curl's
//!   failure, a wrong size or hash (the file is deleted), a bad tar header or
//!   path, a gzip CRC that does not match. The marker is written only after
//!   every step passed.
//! - **NAMED_COMPENSATORS.** Nothing here is irreversible: it downloads and
//!   writes files in a directory it owns. The undo is deleting that
//!   directory, which the command prints.
//! - **DECOMPOSE_BY_SECRETS 2.** No secret, no token, no account: a public
//!   HTTPS download.
//! - **UNCERTAINTY_GATED_HUMANS 1.** skip: a hash either matches or does not;
//!   on a mismatch the person is told to find out why before playing.
//! - **EXTERNAL_VERIFIER 2.** The pinned SHA-256 is the verifier, and the
//!   gzip CRC a second one; the tests unpack archives built in the test and
//!   refuse hostile ones.

use std::ffi::OsString;
use std::fs;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};

/// FreePats' FLAC edition of the Salamander Grand Piano V3.
pub const URL: &str = "https://freepats.zenvoid.org/Piano/SalamanderGrandPiano/\
                       SalamanderGrandPiano-SFZ+FLAC-V3+20200602.tar.gz";
/// The archive's file name.
pub const ARCHIVE: &str = "SalamanderGrandPiano-SFZ+FLAC-V3+20200602.tar.gz";
/// Its size in bytes, as FreePats serves it.
pub const ARCHIVE_BYTES: u64 = 741_757_374;
/// Its SHA-256, measured on the first download (2026-09-26).
pub const ARCHIVE_SHA256: &str = "b7760e168494cf095344e217b0af013fc449ad033abbbdf1c65211cf11dc038b";
/// The archive's one top directory, stripped when it unpacks.
const TOP: &str = "SalamanderGrandPiano-SFZ+FLAC-V3+20200602";
/// The regular files it holds: 641 FLAC samples, two SFZ files and a readme.
pub const FILES: usize = 644;
/// The file written last, once the archive has unpacked.
pub const MARKER: &str = "FETCHED";

/// The operating systems whose cache directories differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    Mac,
    Other,
}

impl Os {
    /// The one this host was built for.
    pub fn this() -> Os {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Other
        }
    }
}

/// The per-user cache directory the piano's samples go to, from the
/// environment `env` reads: `%LOCALAPPDATA%` on Windows,
/// `~/Library/Caches` on macOS, `$XDG_CACHE_HOME` or `~/.cache` elsewhere,
/// each with `si-jam-sessions/salamander-grand-piano-v3` under it.
pub fn default_dir(os: Os, env: impl Fn(&str) -> Option<OsString>) -> Result<PathBuf, String> {
    let absolute = |name: &str| env(name).map(PathBuf::from).filter(|p| p.is_absolute());
    let root = match os {
        Os::Windows => absolute("LOCALAPPDATA"),
        Os::Mac => absolute("HOME").map(|h| h.join("Library").join("Caches")),
        Os::Other => {
            absolute("XDG_CACHE_HOME").or_else(|| absolute("HOME").map(|h| h.join(".cache")))
        }
    };
    let root = root.ok_or_else(|| {
        String::from("no per-user cache directory is set in the environment; name one with --dir")
    })?;
    Ok(root
        .join("si-jam-sessions")
        .join("salamander-grand-piano-v3"))
}

/// The directory the piano plays from: `dir` if one is named, the per-user
/// cache otherwise.
pub fn dir_or_default(dir: Option<&Path>) -> Result<PathBuf, String> {
    match dir {
        Some(d) => Ok(d.to_path_buf()),
        None => default_dir(Os::this(), |name| std::env::var_os(name)),
    }
}

/// Whether `dir` holds a fetch of the pinned archive that finished: its
/// marker names [`ARCHIVE_SHA256`].
pub fn verified(dir: &Path) -> Result<(), String> {
    let marker = dir.join(MARKER);
    let text = fs::read_to_string(&marker).map_err(|_| {
        format!(
            "{} holds no piano samples that `host fetch-piano` finished and verified",
            dir.display()
        )
    })?;
    let pinned = format!("sha256 {ARCHIVE_SHA256}");
    if text.lines().any(|l| l == pinned) {
        Ok(())
    } else {
        Err(format!(
            "{} was fetched from another archive than the one this host pins ({ARCHIVE_SHA256}); \
             run `host fetch-piano` again",
            marker.display()
        ))
    }
}

/// What a fetch did.
#[derive(Debug, PartialEq, Eq)]
pub struct Fetched {
    pub dir: PathBuf,
    /// False when `dir` already held a verified fetch, and nothing was done.
    pub fetched: bool,
    /// The files unpacked.
    pub files: usize,
    /// Where the archive was kept, if it was.
    pub kept: Option<PathBuf>,
}

/// Fetches, verifies and unpacks the samples into `dir` (see the module
/// documentation). `archive` is a copy to use instead of downloading one;
/// `keep` keeps the archive after it unpacks.
pub fn fetch(dir: &Path, archive: Option<&Path>, keep: bool) -> Result<Fetched, String> {
    if verified(dir).is_ok() {
        return Ok(Fetched {
            dir: dir.to_path_buf(),
            fetched: false,
            files: 0,
            kept: None,
        });
    }
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (path, downloaded) = match archive {
        Some(given) => (given.to_path_buf(), false),
        None => {
            let part = dir.join(format!("{ARCHIVE}.part"));
            let have = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
            if have > ARCHIVE_BYTES {
                remove(&part)?;
            }
            if have != ARCHIVE_BYTES {
                println!(
                    "Downloading {URL} ({ARCHIVE_BYTES} bytes) to {}",
                    part.display()
                );
                download(URL, &part)?;
            }
            (part, true)
        }
    };
    let bytes = fs::metadata(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .len();
    let digest = if bytes == ARCHIVE_BYTES {
        sha256_file(&path)?
    } else {
        String::new()
    };
    if digest != ARCHIVE_SHA256 {
        if downloaded {
            remove(&path)?;
        }
        return Err(format!(
            "{} is {bytes} bytes with SHA-256 {}; the host pins {ARCHIVE_BYTES} bytes with \
             SHA-256 {ARCHIVE_SHA256}. It is not the archive the piano was built for, and \
             nothing was unpacked{}.",
            path.display(),
            if digest.is_empty() {
                "(not hashed)"
            } else {
                &digest
            },
            if downloaded {
                "; the download was deleted"
            } else {
                ""
            }
        ));
    }
    println!("Verified {}: SHA-256 {ARCHIVE_SHA256}", path.display());
    let file = fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let files = unpack(file, dir)?;
    if files != FILES {
        return Err(format!(
            "the archive unpacked {files} files, not the {FILES} it holds"
        ));
    }
    let marker = format!(
        "si-jam-sessions fetch-piano\narchive {ARCHIVE}\nsha256 {ARCHIVE_SHA256}\nfiles {files}\n"
    );
    fs::write(dir.join(MARKER), marker).map_err(|e| format!("{}: {e}", dir.display()))?;
    let kept = if keep {
        let final_path = dir.join(ARCHIVE);
        if downloaded {
            fs::rename(&path, &final_path).map_err(|e| format!("{}: {e}", path.display()))?;
            Some(final_path)
        } else {
            Some(path)
        }
    } else {
        if downloaded {
            remove(&path)?;
        }
        None
    };
    Ok(Fetched {
        dir: dir.to_path_buf(),
        fetched: true,
        files,
        kept,
    })
}

fn remove(path: &Path) -> Result<(), String> {
    fs::remove_file(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Downloads `url` to `to` with the system's curl, resuming a partial file,
/// failing on an HTTP error, and retrying a dropped connection three times.
fn download(url: &str, to: &Path) -> Result<(), String> {
    let status = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--retry",
            "3",
            "--continue-at",
            "-",
            "--progress-bar",
            "--output",
        ])
        .arg(to)
        .arg(url)
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("curl stopped ({s}) while downloading {url}")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(format!(
            "there is no `curl` on this system to download with; download {url} some other way, \
             then run `host fetch-piano --archive <the file>`"
        )),
        Err(e) => Err(format!("curl did not start: {e}")),
    }
}

/// The SHA-256 of a file, in lowercase hex.
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(buffer.get(..n).unwrap_or(&[]));
    }
    let digest: [u8; 32] = hasher.finalize().into();
    Ok(golden::run::hex(&digest))
}

/// Unpacks a gzipped tar into `into` and returns the regular files it wrote.
/// Every entry must be a regular file or a directory under [`TOP`]; see the
/// module documentation for what is refused.
pub fn unpack(archive: impl Read, into: &Path) -> Result<usize, String> {
    let mut tar = GzDecoder::new(BufReader::with_capacity(1 << 20, archive));
    let mut header = [0u8; 512];
    let mut files = 0usize;
    loop {
        tar.read_exact(&mut header)
            .map_err(|e| format!("the archive ends inside a tar header: {e}"))?;
        if header.iter().all(|&b| b == 0) {
            break;
        }
        let entry = Entry::read(&header)?;
        let relative = inside(&entry.path)?;
        let target = relative
            .iter()
            .fold(into.to_path_buf(), |path, part| path.join(part));
        match entry.kind {
            Kind::Directory => {
                fs::create_dir_all(&target).map_err(|e| format!("{}: {e}", target.display()))?;
            }
            Kind::File => {
                if relative.is_empty() {
                    return Err(format!("{}: a file where the top directory is", entry.path));
                }
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
                }
                let mut out =
                    fs::File::create(&target).map_err(|e| format!("{}: {e}", target.display()))?;
                let copied = io::copy(&mut (&mut tar).take(entry.size), &mut out)
                    .map_err(|e| format!("{}: {e}", entry.path))?;
                if copied != entry.size {
                    return Err(format!("{}: the archive ends inside it", entry.path));
                }
                files += 1;
            }
        }
        let padding = (512 - entry.size % 512) % 512;
        if entry.kind == Kind::File && padding > 0 {
            io::copy(&mut (&mut tar).take(padding), &mut io::sink())
                .map_err(|e| format!("{}: {e}", entry.path))?;
        }
    }
    // Read the stream to its end: flate2 checks the CRC-32 there.
    io::copy(&mut tar, &mut io::sink()).map_err(|e| format!("the archive's gzip stream: {e}"))?;
    Ok(files)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    File,
    Directory,
}

/// One tar header, read and checked.
struct Entry {
    path: String,
    size: u64,
    kind: Kind,
}

impl Entry {
    fn read(header: &[u8; 512]) -> Result<Entry, String> {
        let text = |range: std::ops::Range<usize>| -> Result<String, String> {
            let bytes = header.get(range).unwrap_or(&[]);
            let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
            String::from_utf8(bytes.get(..end).unwrap_or(&[]).to_vec())
                .map_err(|_| String::from("a tar header whose name is not UTF-8"))
        };
        let name = text(0..100)?;
        let stored = octal(header.get(148..156).unwrap_or(&[]))
            .ok_or_else(|| format!("{name}: a tar header with no checksum"))?;
        let sum: u64 = header
            .iter()
            .enumerate()
            .map(|(i, &b)| {
                if (148..156).contains(&i) {
                    32
                } else {
                    u64::from(b)
                }
            })
            .sum();
        if sum != stored {
            return Err(format!("{name}: the tar header's checksum is wrong"));
        }
        // POSIX ustar keeps a prefix for long paths; GNU's "ustar  " does not.
        let prefix = if header.get(257..263) == Some(b"ustar\0") {
            text(345..500)?
        } else {
            String::new()
        };
        let path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let size = octal(header.get(124..136).unwrap_or(&[]))
            .ok_or_else(|| format!("{path}: a tar header with no size"))?;
        let kind = match header.get(156).copied().unwrap_or(0) {
            b'0' | 0 => Kind::File,
            b'5' => Kind::Directory,
            other => {
                return Err(format!(
                    "{path}: a tar entry of type '{}', which is not a file or a directory",
                    char::from(other)
                ));
            }
        };
        Ok(Entry { path, size, kind })
    }
}

/// An octal tar field: digits, then a NUL or space. `None` for anything else.
fn octal(field: &[u8]) -> Option<u64> {
    let digits: Vec<u8> = field
        .iter()
        .copied()
        .skip_while(|&b| b == b' ')
        .take_while(|&b| b != 0 && b != b' ')
        .collect();
    if digits.is_empty() || !digits.iter().all(|b| (b'0'..=b'7').contains(b)) {
        return None;
    }
    digits.iter().try_fold(0u64, |n, &d| {
        n.checked_mul(8)?.checked_add(u64::from(d - b'0'))
    })
}

/// A tar path's parts under [`TOP`], or why it is refused.
fn inside(path: &str) -> Result<Vec<String>, String> {
    let mut parts = path.split('/').filter(|p| !p.is_empty());
    if parts.next() != Some(TOP) || path.starts_with('/') {
        return Err(format!(
            "{path}: not under the archive's top directory {TOP}"
        ));
    }
    let allowed = |c: char| c.is_ascii_alphanumeric() || "+#._-".contains(c);
    parts
        .map(|p| {
            if p == "." || p == ".." || !p.chars().all(allowed) {
                Err(format!("{path}: a path the piano does not unpack"))
            } else {
                Ok(p.to_owned())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    /// A tar header, GNU style as the real archive has them.
    fn header(path: &str, size: usize, kind: u8) -> [u8; 512] {
        let mut h = [0u8; 512];
        h[..path.len()].copy_from_slice(path.as_bytes());
        h[100..108].copy_from_slice(b"0000644\0");
        h[108..116].copy_from_slice(b"0001750\0");
        h[116..124].copy_from_slice(b"0001750\0");
        h[124..136].copy_from_slice(format!("{size:011o}\0").as_bytes());
        h[136..148].copy_from_slice(b"13665437276\0");
        h[156] = kind;
        h[257..265].copy_from_slice(b"ustar  \0");
        h[148..156].copy_from_slice(b"        ");
        let sum: u32 = h.iter().map(|&b| u32::from(b)).sum();
        h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
        h
    }

    /// A gzipped tar of `entries`: (path, contents, type).
    fn archive(entries: &[(&str, &[u8], u8)]) -> Vec<u8> {
        let mut tar = Vec::new();
        for (path, contents, kind) in entries {
            tar.extend_from_slice(&header(path, contents.len(), *kind));
            tar.extend_from_slice(contents);
            tar.resize(tar.len().div_ceil(512) * 512, 0);
        }
        tar.extend_from_slice(&[0; 1024]);
        let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
        gz.write_all(&tar).unwrap();
        gz.finish().unwrap()
    }

    /// A fresh directory under the system's temporary directory.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("si-jam-fetch-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn top(rest: &str) -> String {
        format!("{TOP}/{rest}")
    }

    /// Files and directories under the top directory unpack, the top
    /// stripped, with their bytes, across block padding.
    #[test]
    fn an_archive_unpacks_its_files_under_the_top_directory() {
        let dir = scratch("unpack");
        let long = vec![7u8; 1_300];
        let bytes = archive(&[
            (&top(""), b"", b'5'),
            (&top("samples/"), b"", b'5'),
            (&top("samples/D#1v8.flac"), &long, b'0'),
            (&top("readme.txt"), b"hello", b'0'),
            (&top("SalamanderGrandPiano-V3+20200602.sfz"), b"", b'0'),
        ]);
        assert_eq!(unpack(&bytes[..], &dir), Ok(3));
        assert_eq!(
            fs::read(dir.join("samples").join("D#1v8.flac")).unwrap(),
            long
        );
        assert_eq!(fs::read(dir.join("readme.txt")).unwrap(), b"hello");
        assert!(dir.join("SalamanderGrandPiano-V3+20200602.sfz").is_file());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// What must not be unpacked is refused, and nothing is written outside
    /// the directory: a climb out, an absolute path, a link, another top, a
    /// name with a backslash or a drive, a wrong header checksum.
    #[test]
    fn a_hostile_archive_is_refused() {
        // The target sits inside a directory of this run's own, so a climb
        // out of it lands where only this run looks.
        let root = scratch("hostile");
        let dir = root.join("into");
        fs::create_dir_all(&dir).unwrap();
        let cases: Vec<(String, u8, &str)> = vec![
            (
                top("../escape.txt"),
                b'0',
                "a path the piano does not unpack",
            ),
            (
                String::from("/etc/passwd"),
                b'0',
                "not under the archive's top",
            ),
            (top("samples/link"), b'2', "type '2'"),
            (top("samples/hard"), b'1', "type '1'"),
            (
                String::from("other/readme.txt"),
                b'0',
                "not under the archive's top",
            ),
            (
                top("samples\\..\\x"),
                b'0',
                "a path the piano does not unpack",
            ),
            (top("C:x"), b'0', "a path the piano does not unpack"),
        ];
        for (path, kind, why) in cases {
            let bytes = archive(&[(&path, b"x", kind)]);
            let e = unpack(&bytes[..], &dir).err().unwrap();
            assert!(e.contains(why), "{path}: {e}");
        }
        let mut bad = archive(&[(&top("readme.txt"), b"x", b'0')]);
        // Re-compress a tar whose header has one byte changed.
        let mut tar = Vec::new();
        GzDecoder::new(&bad[..]).read_to_end(&mut tar).unwrap();
        tar[10] ^= 1;
        let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
        gz.write_all(&tar).unwrap();
        bad = gz.finish().unwrap();
        let e = unpack(&bad[..], &dir).err().unwrap();
        assert!(e.contains("checksum is wrong"), "{e}");
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            0,
            "nothing was written"
        );
        assert!(!root.join("escape.txt").exists(), "nothing climbed out");
        fs::remove_dir_all(&root).unwrap();
    }

    /// A gzip stream whose data does not match its CRC-32 is refused.
    #[test]
    fn a_corrupt_gzip_stream_is_refused() {
        let dir = scratch("crc");
        let mut bytes = archive(&[(&top("readme.txt"), b"hello", b'0')]);
        let crc = bytes.len() - 8;
        bytes[crc] ^= 0xFF;
        let e = unpack(&bytes[..], &dir).err().unwrap();
        assert!(e.contains("gzip"), "{e}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// An archive that is not the pinned one, by size or by hash, is refused
    /// and nothing is unpacked; a directory without a finished fetch does not
    /// verify.
    #[test]
    fn only_the_pinned_archive_unpacks() {
        let dir = scratch("pinned");
        let given = dir.join("given.tar.gz");
        fs::write(&given, archive(&[(&top("readme.txt"), b"x", b'0')])).unwrap();
        let e = fetch(&dir.join("piano"), Some(&given), false)
            .err()
            .unwrap();
        assert!(
            e.contains("It is not the archive the piano was built for"),
            "{e}"
        );
        assert!(e.contains(ARCHIVE_SHA256), "{e}");
        assert!(given.exists(), "a copy the person gave is not deleted");
        assert!(!dir.join("piano").join("readme.txt").exists());
        assert!(verified(&dir.join("piano")).is_err());
        // A marker that names another archive does not verify either.
        fs::write(dir.join("piano").join(MARKER), "sha256 00\n").unwrap();
        let e = verified(&dir.join("piano")).err().unwrap();
        assert!(e.contains("another archive"), "{e}");
        fs::write(
            dir.join("piano").join(MARKER),
            format!("sha256 {ARCHIVE_SHA256}\n"),
        )
        .unwrap();
        assert_eq!(verified(&dir.join("piano")), Ok(()));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The pinned URL is one line, and names the pinned archive.
    #[test]
    fn the_url_names_the_archive() {
        assert!(URL.starts_with("https://freepats.zenvoid.org/"));
        assert!(URL.ends_with(&format!("/{ARCHIVE}")));
        assert!(!URL.contains(char::is_whitespace));
        assert_eq!(ARCHIVE_SHA256.len(), 64);
    }

    #[test]
    fn a_file_hashes_to_its_sha256() {
        let dir = scratch("hash");
        let path = dir.join("abc");
        fs::write(&path, b"abc").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The cache directory follows each system's convention, and a relative
    /// or missing variable is not trusted.
    #[test]
    fn the_cache_directory_follows_the_system() {
        let tail = Path::new("si-jam-sessions").join("salamander-grand-piano-v3");
        let root = std::env::temp_dir();
        let env = |pairs: Vec<(&'static str, PathBuf)>| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, v)| v.clone().into_os_string())
            }
        };
        let r = root.clone();
        assert_eq!(
            default_dir(Os::Windows, env(vec![("LOCALAPPDATA", r.clone())])),
            Ok(r.join(&tail))
        );
        assert_eq!(
            default_dir(Os::Mac, env(vec![("HOME", r.clone())])),
            Ok(r.join("Library").join("Caches").join(&tail))
        );
        assert_eq!(
            default_dir(
                Os::Other,
                env(vec![("XDG_CACHE_HOME", r.join("c")), ("HOME", r.clone())])
            ),
            Ok(r.join("c").join(&tail))
        );
        assert_eq!(
            default_dir(Os::Other, env(vec![("HOME", r.clone())])),
            Ok(r.join(".cache").join(&tail))
        );
        assert!(default_dir(Os::Other, env(vec![("HOME", PathBuf::from("relative"))])).is_err());
        assert!(default_dir(Os::Windows, env(vec![])).is_err());
    }
}
