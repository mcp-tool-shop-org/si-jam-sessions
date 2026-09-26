//! The licences of the crates the host is built from, and of the samples its
//! piano plays, which `host notices` prints. cpal is Apache-2.0 only, so a
//! recipient of the host gets the licence's text (KB recipe 1524); alsa-sys,
//! on Linux, is MIT only, so its notice goes too (KB recipe 1528). The
//! piano's samples are CC BY 3.0, so their credit and the licence's legal code
//! go with the host, though the samples themselves are fetched, not shipped.

/// The notices, the Apache-2.0 text (cpal's own `LICENSE`, unchanged),
/// alsa-sys's MIT notice, and the piano samples' notice with the CC BY 3.0
/// legal code.
pub const TEXT: &str = concat!(
    include_str!("../licences/NOTICES.txt"),
    "\n---------------------------------------------------------------------\n\n",
    include_str!("../licences/Apache-2.0.txt"),
    "\n---------------------------------------------------------------------\n\n",
    "alsa-sys 0.4.0:\n\n",
    include_str!("../licences/alsa-sys-MIT.txt"),
    "\n---------------------------------------------------------------------\n\n",
    "simd-adler32 0.3.10:\n\n",
    include_str!("../licences/simd-adler32-MIT.txt"),
    "\n---------------------------------------------------------------------\n\n",
    include_str!("../licences/Salamander-NOTICE.txt"),
    "\n",
    include_str!("../licences/CC-BY-3.0.txt"),
);

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;
    use std::process::Command;

    /// The crates `cargo tree` says are linked into the host for `target`,
    /// with their licences: `name version licence`. The workspace's own
    /// crates, build-only crates and procedural macros are left out.
    fn linked(target: &str) -> BTreeSet<String> {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = Command::new(cargo)
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
            .args([
                "tree",
                "-p",
                "host",
                "--locked",
                "--target",
                target,
                "-e",
                "normal,no-proc-macro",
                "--prefix",
                "none",
                "--format",
                "{p} {l}",
            ])
            .output()
            .expect("cargo tree runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .filter(|l| !l.contains(" (") || l.ends_with(" (*)"))
            .map(|l| l.trim_end_matches(" (*)").replacen(" v", " ", 1))
            .filter(|l| {
                let name = l.split(' ').next().unwrap_or("");
                ![
                    "host",
                    "law",
                    "golden",
                    "provenance",
                    "ingest",
                    "score-model",
                ]
                .contains(&name)
            })
            .collect()
    }

    /// The notices' list for one target: its indented lines after the target's
    /// heading, as `name version licence`.
    fn listed(heading: &str) -> BTreeSet<String> {
        super::TEXT
            .lines()
            .skip_while(|l| !l.starts_with(heading))
            .skip(1)
            .take_while(|l| l.starts_with("  "))
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect()
    }

    /// Every crate linked into the host, on both targets, is listed with its
    /// version and its licence, and nothing else is: a new dependency cannot
    /// ship without its notice.
    #[test]
    fn the_notices_list_every_linked_crate() {
        assert_eq!(listed("Windows"), linked("x86_64-pc-windows-msvc"));
        assert_eq!(listed("Linux"), linked("x86_64-unknown-linux-gnu"));
        let licences: BTreeSet<String> = listed("Windows")
            .union(&listed("Linux"))
            .map(|l| l.splitn(3, ' ').nth(2).unwrap_or("").to_owned())
            .collect();
        let expected: BTreeSet<String> = [
            "0BSD OR MIT OR Apache-2.0",
            "Apache-2.0",
            "Apache-2.0/MIT",
            "MIT",
            "MIT OR Apache-2.0",
            "MIT OR Zlib OR Apache-2.0",
            "Unlicense",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(licences, expected, "a licence the notices do not cover");
    }

    #[test]
    fn the_notices_carry_the_licence_texts() {
        assert!(super::TEXT.contains("Apache License\n                           Version 2.0"));
        assert!(super::TEXT.contains("Copyright (c) 2018 diwic"));
        assert!(super::TEXT.contains("Copyright (c) [2021] [Marvin Countryman]"));
        assert!(super::TEXT.contains("Permission is hereby granted, free of charge"));
    }

    /// The piano's credit is in the notices word for word, with the CC BY 3.0
    /// legal code, byte for byte the file the research manifest recorded.
    #[test]
    fn the_notices_carry_the_pianos_credit_and_licence() {
        let credit = crate::piano::CREDIT.split_whitespace().collect::<Vec<_>>();
        let text = super::TEXT.split_whitespace().collect::<Vec<_>>();
        assert!(
            text.windows(credit.len()).any(|w| w == credit.as_slice()),
            "the credit"
        );
        let legal = include_bytes!("../licences/CC-BY-3.0.txt");
        assert_eq!(
            golden::run::hex(&golden::run::sha256(legal)),
            "e6bc9e9c474700b708f568bac9e5a8a9bcb2b1dad53442f5ba449fcb848b8e76"
        );
        assert!(super::TEXT.contains("Attribution 3.0 Unported"));
    }
}
