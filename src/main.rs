//! replant: moves hard-coded FHS paths in a source tree to the Zainium OS layout.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const BASE: &str = "/overlayer/syshub";

/// Old prefix and where it lives on Zainium OS; the first match wins.
const MAP: &[(&str, &str)] = &[
    ("/usr/local/bin", "/overlayer/syshub/bin"),
    ("/usr/local/sbin", "/overlayer/syshub/sbin"),
    ("/usr/local/lib", "/overlayer/syshub/lib"),
    ("/usr/local/share", "/overlayer/syshub/share"),
    ("/usr/local/include", "/overlayer/syshub/include"),
    ("/usr/local/etc", "/overlayer/syshub/etc"),
    ("/usr/bin", "/overlayer/syshub/bin"),
    ("/usr/sbin", "/overlayer/syshub/sbin"),
    ("/usr/lib64", "/overlayer/syshub/lib"),
    ("/usr/libexec", "/overlayer/syshub/libexec"),
    ("/usr/lib", "/overlayer/syshub/lib"),
    ("/usr/share", "/overlayer/syshub/share"),
    ("/usr/include", "/overlayer/syshub/include"),
    ("/usr/local", "/overlayer/syshub"),
    ("/usr", "/overlayer/syshub"),
    (
        "/lib/firmware",
        "/overlayer/syshub/drivers/hardware/firmwares",
    ),
    ("/lib/modules", "/overlayer/syshub/drivers/modules"),
    ("/lib64", "/overlayer/syshub/lib"),
    ("/lib", "/overlayer/syshub/lib"),
    ("/bin", "/overlayer/syshub/bin"),
    ("/sbin", "/overlayer/syshub/sbin"),
    ("/etc", "/overlayer/syshub/etc"),
    ("/var", "/overlayer/syshub/var"),
];

#[derive(Clone, Copy, PartialEq)]
enum Lang {
    CLike,
    Rust,
    Hash,
    Plain,
}

fn lang_of(path: &Path) -> Option<Lang> {
    let name = path.file_name()?.to_str()?;
    let name = name.strip_suffix(".in").unwrap_or(name);
    let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "c" | "h" | "cc" | "cpp" | "cxx" | "hh" | "hpp" | "go" | "vala" | "js" | "ts" => {
            Some(Lang::CLike)
        }
        "rs" => Some(Lang::Rust),
        "py" | "sh" | "bash" | "pl" | "rb" => Some(Lang::Hash),
        "conf" | "service" | "socket" | "target" | "timer" | "path" | "mount" | "automount"
        | "slice" | "scope" | "network" | "netdev" | "link" | "preset" | "rules" | "desktop" => {
            Some(Lang::Plain)
        }
        _ if is_man(name) => Some(Lang::Plain),
        _ => None,
    }
}

/// Man pages (`patch.man`, `ls.1`, `printf.3p`): they are installed, so their paths count.
fn is_man(name: &str) -> bool {
    let Some((_, ext)) = name.rsplit_once('.') else {
        return false;
    };
    ext == "man"
        || (ext.len() <= 3
            && ext.starts_with(|c: char| ('1'..='9').contains(&c))
            && ext[1..].chars().all(|c| c.is_ascii_lowercase()))
}

/// Doc folders are skipped except for their man pages.
fn doc_dir(name: &str) -> bool {
    matches!(name, "doc" | "docs" | "man")
}

/// Paths in tests, docs and translations are not changed.
fn skipped_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "test"
            | "tests"
            | "testsuite"
            | "doc"
            | "docs"
            | "po"
            | "man"
            | "examples"
            | "target"
            | "build"
            | "builddir"
            | "vendor"
            | "third_party"
            | "3rdparty"
    )
}

fn files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), false)];
    while let Some((dir, man_only)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.starts_with('.') || n.starts_with("mkosi") {
                    continue;
                }
                if !skipped_dir(&n) {
                    stack.push((p, man_only));
                } else if doc_dir(&n) {
                    stack.push((p, true));
                }
            } else if ft.is_file()
                && lang_of(&p).is_some()
                && (!man_only || is_man(&e.file_name().to_string_lossy()))
                && !e.file_name().to_string_lossy().starts_with("mkosi")
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Byte ranges of the lines of a config file that are not comments.
fn config_lines(src: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut at = 0;
    for line in src.split_inclusive('\n') {
        let t = line.trim_start();
        if !t.starts_with('#') && !t.starts_with(';') && !t.starts_with(".\\\"") {
            out.push((at, at + line.trim_end_matches('\n').len()));
        }
        at += line.len();
    }
    out
}

/// Byte ranges of the string literals in `src`, skipping comments.
fn literals(src: &str, lang: Lang) -> Vec<(usize, usize)> {
    if lang == Lang::Plain {
        return config_lines(src);
    }
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if lang != Lang::Hash && c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if lang != Lang::Hash && c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else if lang == Lang::Hash && c == b'#' && (i == 0 || b[i - 1] != b'$') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'"'
            || (c == b'\'' && lang == Lang::Hash)
            || (c == b'`' && lang == Lang::CLike)
        {
            let start = i + 1;
            i += 1;
            while i < b.len() && b[i] != c {
                if b[i] == b'\\' && c != b'\'' && c != b'`' {
                    i += 1;
                }
                if i < b.len() && b[i] == b'\n' && c == b'"' && lang != Lang::Hash {
                    break;
                }
                i += 1;
            }
            out.push((start, i.min(b.len())));
            i += 1;
        } else if c == b'\'' && lang != Lang::Hash {
            i += 1;
            while i < b.len() && b[i] != b'\'' && b[i] != b'\n' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    out
}

fn is_name_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b'.'
}

/// The mapping for a path that starts at `s`, if one applies: (old, new).
fn rewrite_at(s: &str) -> Option<(&'static str, &'static str)> {
    MAP.iter()
        .find(|(old, _)| {
            s.starts_with(old)
                && s.as_bytes()
                    .get(old.len())
                    .is_none_or(|&c| !is_name_byte(c))
        })
        .copied()
}

/// A change: byte offset in the file, old prefix, new prefix.
struct Change {
    at: usize,
    old: &'static str,
    new: &'static str,
}

fn changes(src: &str, lang: Lang) -> Vec<Change> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    for (start, end) in literals(src, lang) {
        let mut i = start;
        while i < end {
            let before_ok = i == start
                || matches!(
                    b[i - 1],
                    b' ' | b':' | b'=' | b'\t' | b'(' | b',' | b'|' | b'!' | b'-' | b'+' | b'@'
                );
            let url = i >= 2 && &b[i - 2..i] == b":/";
            if b[i] == b'/' && before_ok && !url && !src[i..end].starts_with(BASE) {
                if let Some((old, new)) = rewrite_at(&src[i..end]) {
                    out.push(Change { at: i, old, new });
                    i += old.len();
                    continue;
                }
            }
            i += 1;
        }
    }
    out
}

fn apply(src: &str, ch: &[Change]) -> String {
    let mut out = String::with_capacity(src.len());
    let mut last = 0;
    for c in ch {
        out.push_str(&src[last..c.at]);
        out.push_str(c.new);
        last = c.at + c.old.len();
    }
    out.push_str(&src[last..]);
    out
}

fn diff_line(out: &mut String, mark: char, line: &str) {
    out.push(mark);
    out.push_str(line);
    if !line.ends_with('\n') {
        out.push_str("\n\\ No newline at end of file\n");
    }
}

/// A unified diff of two texts with the same number of lines.
fn diff(rel: &str, a: &str, b: &str) -> String {
    let al: Vec<&str> = a.split_inclusive('\n').collect();
    let bl: Vec<&str> = b.split_inclusive('\n').collect();
    let changed: Vec<usize> = (0..al.len()).filter(|&i| al[i] != bl[i]).collect();
    if changed.is_empty() {
        return String::new();
    }
    let mut out = format!("--- a/{rel}\n+++ b/{rel}\n");
    let mut k = 0;
    while k < changed.len() {
        let lo = changed[k].saturating_sub(3);
        let mut hi = (changed[k] + 4).min(al.len());
        let mut j = k + 1;
        while j < changed.len() && changed[j] <= hi + 3 {
            hi = (changed[j] + 4).min(al.len());
            j += 1;
        }
        let n = hi - lo;
        out.push_str(&format!("@@ -{},{n} +{},{n} @@\n", lo + 1, lo + 1));
        for i in lo..hi {
            if al[i] == bl[i] {
                diff_line(&mut out, ' ', al[i]);
            } else {
                diff_line(&mut out, '-', al[i]);
                diff_line(&mut out, '+', bl[i]);
            }
        }
        k = j;
    }
    out
}

struct Found {
    rel: String,
    src: String,
    changes: Vec<Change>,
}

fn scan(root: &Path) -> Vec<Found> {
    let mut out = Vec::new();
    for f in files(root) {
        let Ok(src) = fs::read_to_string(&f) else {
            continue;
        };
        let ch = changes(&src, lang_of(&f).unwrap());
        if !ch.is_empty() {
            let rel = f
                .strip_prefix(root)
                .unwrap_or(&f)
                .to_string_lossy()
                .into_owned();
            out.push(Found {
                rel,
                src,
                changes: ch,
            });
        }
    }
    out
}

fn line_of(src: &str, at: usize) -> usize {
    src[..at].bytes().filter(|&c| c == b'\n').count() + 1
}

fn usage() -> ExitCode {
    eprintln!("usage: replant map");
    eprintln!("       replant find <source-dir>");
    eprintln!("       replant patch <source-dir> [-o 0001-zainium-paths.patch]");
    eprintln!("       replant verify <source-dir>");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = || args.get(1).map(PathBuf::from);
    match args.first().map(String::as_str) {
        Some("map") => {
            for (from, to) in MAP {
                println!("{from:<20} {to}");
            }
            ExitCode::SUCCESS
        }
        Some("find") => {
            let Some(d) = dir() else { return usage() };
            let found = scan(&d);
            let mut n = 0;
            for f in &found {
                for c in &f.changes {
                    let end = f.src[c.at..]
                        .find(['"', '\'', '`', '\n'])
                        .map_or(f.src.len(), |e| c.at + e);
                    println!(
                        "{}:{}: {}  ->  {}{}",
                        f.rel,
                        line_of(&f.src, c.at),
                        &f.src[c.at..end],
                        c.new,
                        &f.src[c.at + c.old.len()..end]
                    );
                    n += 1;
                }
            }
            eprintln!("{n} path(s) in {} file(s)", found.len());
            ExitCode::SUCCESS
        }
        Some("patch") => {
            let Some(d) = dir() else { return usage() };
            let out = args
                .iter()
                .position(|a| a == "-o")
                .and_then(|i| args.get(i + 1));
            let mut patch = String::new();
            let mut n = 0;
            for f in scan(&d) {
                n += f.changes.len();
                patch.push_str(&diff(&f.rel, &f.src, &apply(&f.src, &f.changes)));
            }
            match out {
                Some(o) => {
                    if let Err(e) = fs::write(o, &patch) {
                        eprintln!("replant: {o}: {e}");
                        return ExitCode::FAILURE;
                    }
                    eprintln!("{n} path(s) written to {o}");
                }
                None => print!("{patch}"),
            }
            ExitCode::SUCCESS
        }
        Some("verify") => {
            let Some(d) = dir() else { return usage() };
            let left: usize = scan(&d).iter().map(|f| f.changes.len()).sum();
            if left == 0 {
                println!("ok: no hard-coded FHS paths left");
                ExitCode::SUCCESS
            } else {
                println!("{left} hard-coded FHS path(s) left, see: replant find");
                ExitCode::FAILURE
            }
        }
        _ => usage(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(src: &str, lang: Lang) -> String {
        apply(src, &changes(src, lang))
    }

    #[test]
    fn moves_paths_in_strings_only() {
        let src = "// reads /etc/passwd\nconst char *p = \"/etc/passwd\";\n";
        let want = "// reads /etc/passwd\nconst char *p = \"/overlayer/syshub/etc/passwd\";\n";
        assert_eq!(fix(src, Lang::CLike), want);
    }

    #[test]
    fn whole_segments_and_longest_prefix() {
        assert_eq!(
            fix("x = \"/etcd/conf\"\n", Lang::Hash),
            "x = \"/etcd/conf\"\n"
        );
        assert_eq!(
            fix("x = \"/usr/local/share/a\"\n", Lang::Hash),
            "x = \"/overlayer/syshub/share/a\"\n"
        );
        assert_eq!(
            fix("x = \"/lib/firmware\"\n", Lang::Hash),
            "x = \"/overlayer/syshub/drivers/hardware/firmwares\"\n"
        );
    }

    #[test]
    fn leaves_urls_runtime_dirs_and_done_paths() {
        let url = "u = \"https://x.org/etc/a\"\n";
        assert_eq!(fix(url, Lang::Hash), url);
        let run = "p = \"/proc/self\" ; q = \"/tmp/x\"\n";
        assert_eq!(fix(run, Lang::Hash), run);
        let done = "p = \"/overlayer/syshub/etc\"\n";
        assert_eq!(fix(done, Lang::Hash), done);
    }

    #[test]
    fn config_files_outside_comments() {
        let src = "# keeps /var/log\nd /var/log 0755 - - -\nExecStart=/usr/bin/foo\n";
        let want = "# keeps /var/log\nd /overlayer/syshub/var/log 0755 - - -\nExecStart=/overlayer/syshub/bin/foo\n";
        assert_eq!(fix(src, Lang::Plain), want);
    }

    #[test]
    fn systemd_condition_prefixes() {
        let src = "ConditionNeedsUpdate=|/var\nExecStart=-/usr/bin/true\n";
        let want =
            "ConditionNeedsUpdate=|/overlayer/syshub/var\nExecStart=-/overlayer/syshub/bin/true\n";
        assert_eq!(fix(src, Lang::Plain), want);
    }

    #[test]
    fn path_lists() {
        assert_eq!(
            fix("PATH=\"/bin:/usr/bin\"\n", Lang::Hash),
            "PATH=\"/overlayer/syshub/bin:/overlayer/syshub/bin\"\n"
        );
    }

    #[test]
    fn diff_is_a_valid_hunk() {
        let a = "1\n2\n\"/etc/x\"\n4\n";
        let b = "1\n2\n\"/overlayer/syshub/etc/x\"\n4\n";
        let want = "--- a/f.c\n+++ b/f.c\n@@ -1,4 +1,4 @@\n 1\n 2\n-\"/etc/x\"\n+\"/overlayer/syshub/etc/x\"\n 4\n";
        assert_eq!(diff("f.c", a, b), want);
    }
}
