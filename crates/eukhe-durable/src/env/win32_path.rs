//! Node's `path.win32` functions, `os.tmpdir()` and `url.fileURLToPath()`
//! on Windows, as `env/node.ts` uses them there: drive letters, UNC roots,
//! both separators accepted, `\` written. Compiled on Windows, and in tests
//! everywhere so the rules are checked on every host.

/// Either Windows separator.
fn is_sep(byte: u8) -> bool {
    byte == b'/' || byte == b'\\'
}

/// The root of `path`: its device (`C:` or `\\server\share`, else empty),
/// the byte length of the whole root (device plus a separator), and whether
/// the path is absolute. Node's root parse in `win32.resolve`/`normalize`.
fn parse_root(path: &str) -> (String, usize, bool) {
    let bytes = path.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return (String::new(), 0, false);
    }
    if is_sep(bytes[0]) {
        if len > 1 && is_sep(bytes[1]) {
            // `\\server\share`: a UNC root is absolute.
            let mut j = 2;
            while j < len && !is_sep(bytes[j]) {
                j += 1;
            }
            if j < len && j != 2 {
                let server = &path[2..j];
                let mut k = j;
                while k < len && is_sep(bytes[k]) {
                    k += 1;
                }
                let mut m = k;
                while m < len && !is_sep(bytes[m]) {
                    m += 1;
                }
                if m != k {
                    let share = &path[k..m];
                    // The separator after the share belongs to the root.
                    let root_end = if m < len { m + 1 } else { m };
                    return (format!("\\\\{server}\\{share}"), root_end, true);
                }
            }
        }
        return (String::new(), 1, true);
    }
    if len >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let device = path[..2].to_owned();
        if len > 2 && is_sep(bytes[2]) {
            return (device, 3, true);
        }
        return (device, 2, false);
    }
    (String::new(), 0, false)
}

/// The segments of `path` after removing `.`, empty segments and resolving
/// `..`, joined with `\`; with `allow_above_root`, leading `..` are kept.
fn normalize_segments(path: &str, allow_above_root: bool) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split(['/', '\\']) {
        match segment {
            "" | "." => {}
            ".." => {
                if segments.last().is_some_and(|last| *last != "..") {
                    segments.pop();
                } else if allow_above_root {
                    segments.push("..");
                }
            }
            name => segments.push(name),
        }
    }
    segments.join("\\")
}

/// `process.cwd()`, or `C:\` when it cannot be read.
fn process_cwd() -> String {
    std::env::current_dir().map_or_else(
        |_| "C:\\".to_owned(),
        |path| path.to_string_lossy().into_owned(),
    )
}

/// Whether two devices name the same drive or share (case-insensitively).
fn same_device(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Node's `path.win32.resolve(...paths)`. A drive-relative path of another
/// drive resolves against that drive's root (Node reads the drive's own
/// cwd from a hidden `=X:` variable a native process does not carry).
pub(crate) fn resolve(paths: &[&str]) -> String {
    let cwd = process_cwd();
    let mut device = String::new();
    let mut tail = String::new();
    let mut absolute = false;
    let candidates = paths
        .iter()
        .rev()
        .copied()
        .chain(std::iter::once(cwd.as_str()));
    for path in candidates {
        if path.is_empty() {
            continue;
        }
        let (this_device, root_end, this_absolute) = parse_root(path);
        if !this_device.is_empty() {
            if device.is_empty() {
                device = this_device;
            } else if !same_device(&device, &this_device) {
                continue;
            }
        }
        if !absolute {
            tail = format!("{}\\{tail}", &path[root_end..]);
            absolute = this_absolute;
        }
        if absolute && !device.is_empty() {
            break;
        }
    }
    let tail = normalize_segments(&tail, !absolute);
    if absolute {
        format!("{device}\\{tail}")
    } else if device.is_empty() && tail.is_empty() {
        ".".to_owned()
    } else {
        format!("{device}{tail}")
    }
}

/// Node's `path.win32.normalize(path)`.
pub(crate) fn normalize(path: &str) -> String {
    if path.is_empty() {
        return ".".to_owned();
    }
    let (device, root_end, absolute) = parse_root(path);
    let rest = &path[root_end..];
    let trailing_separator = rest.as_bytes().last().is_some_and(|byte| is_sep(*byte));
    let mut tail = normalize_segments(rest, !absolute);
    if tail.is_empty() && !absolute {
        tail.push('.');
    }
    if !tail.is_empty() && trailing_separator {
        tail.push('\\');
    }
    if absolute {
        format!("{device}\\{tail}")
    } else {
        format!("{device}{tail}")
    }
}

/// Node's `path.win32.join(...paths)`.
pub(crate) fn join(paths: &[&str]) -> String {
    let joined = paths
        .iter()
        .filter(|path| !path.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("\\");
    if joined.is_empty() {
        return ".".to_owned();
    }
    normalize(&joined)
}

/// Node's `path.win32.isAbsolute(path)`.
pub(crate) fn is_absolute(path: &str) -> bool {
    parse_root(path).2
}

/// Node's `path.win32.basename(path)`.
pub(crate) fn basename(path: &str) -> &str {
    let bytes = path.as_bytes();
    let start = if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        2
    } else {
        0
    };
    let trimmed = path[start..].trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        return "";
    }
    trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed)
}

/// Node's `path.win32.dirname(path)`.
pub(crate) fn dirname(path: &str) -> &str {
    if path.is_empty() {
        return ".";
    }
    let (_, root_end, _) = parse_root(path);
    let bytes = path.as_bytes();
    let mut end = None;
    let mut matched_sep = true;
    for index in (root_end.max(1)..bytes.len()).rev() {
        if is_sep(bytes[index]) {
            if !matched_sep {
                end = Some(index);
                break;
            }
        } else {
            matched_sep = false;
        }
    }
    match end {
        Some(end) if end >= root_end => &path[..end],
        _ if root_end > 0 => &path[..root_end],
        _ => ".",
    }
}

/// `os.tmpdir()` on Windows: `%TEMP%`, `%TMP%`, `%SystemRoot%\temp` or
/// `%windir%\temp`, without a trailing `\` (except after a drive).
// Reads the process environment, so only Windows builds call it.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn tmp_dir() -> String {
    let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    let mut path = var("TEMP")
        .or_else(|| var("TMP"))
        .or_else(|| var("SystemRoot").map(|root| format!("{root}\\temp")))
        .or_else(|| var("windir").map(|root| format!("{root}\\temp")))
        .unwrap_or_else(|| "C:\\Windows\\temp".to_owned());
    if path.len() > 1 && path.ends_with('\\') && !path.ends_with(":\\") {
        path.pop();
    }
    path
}

/// `url.fileURLToPath(url)` on Windows, or `None` where Node throws: not a
/// `file:` URL, an encoded separator, percent-escapes that are not UTF-8,
/// or a local path without a drive letter. A host makes a UNC path.
pub(crate) fn file_url_to_path(input: &str) -> Option<String> {
    let url = url::Url::parse(input).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    let pathname = url.path();
    let lower = pathname.to_ascii_lowercase();
    if lower.contains("%2f") || lower.contains("%5c") {
        return None;
    }
    let decoded = percent_encoding::percent_decode_str(pathname)
        .decode_utf8()
        .ok()?
        .replace('/', "\\");
    if let Some(host) = url.host_str().filter(|host| !host.is_empty()) {
        return Some(format!("\\\\{host}{decoded}"));
    }
    let bytes = decoded.as_bytes();
    let drive =
        bytes.len() >= 3 && bytes[0] == b'\\' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':';
    drive.then(|| decoded[1..].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win32_path_functions_match_node() {
        assert_eq!(resolve(&["C:\\a\\b", "..\\c\\.\\d\\"]), "C:\\a\\c\\d");
        assert_eq!(resolve(&["C:/a", "D:\\b", "c"]), "D:\\b\\c");
        assert_eq!(resolve(&["C:\\", ".."]), "C:\\");
        assert_eq!(
            resolve(&["C:\\x", "C:\\Users\\t\\.tmp\\session"]),
            "C:\\Users\\t\\.tmp\\session"
        );
        assert_eq!(resolve(&["\\\\srv\\share\\a", "b"]), "\\\\srv\\share\\a\\b");
        assert_eq!(normalize("C:/a//b/../c/"), "C:\\a\\c\\");
        assert_eq!(normalize("a\\..\\..\\b"), "..\\b");
        assert_eq!(normalize("./"), ".\\");
        assert_eq!(normalize("C:"), "C:.");
        assert_eq!(join(&["C:\\x\\y", ".."]), "C:\\x");
        assert_eq!(join(&["a/", "b/"]), "a\\b\\");
        assert_eq!(join(&["", ""]), ".");
        assert!(is_absolute("C:\\a"));
        assert!(is_absolute("C:/a"));
        assert!(is_absolute("\\a"));
        assert!(is_absolute("\\\\srv\\share"));
        assert!(!is_absolute("C:a"));
        assert!(!is_absolute("a\\b"));
        assert_eq!(basename("C:\\a\\b.txt"), "b.txt");
        assert_eq!(basename("C:\\a\\b\\"), "b");
        assert_eq!(basename("C:\\"), "");
        assert_eq!(basename("C:"), "");
        assert_eq!(dirname("C:\\a\\b"), "C:\\a");
        assert_eq!(dirname("C:\\a"), "C:\\");
        assert_eq!(dirname("C:\\"), "C:\\");
        assert_eq!(dirname("\\\\srv\\share\\a"), "\\\\srv\\share\\");
        assert_eq!(dirname("a"), ".");
        assert_eq!(
            file_url_to_path("file:///C:/dir/file%20with%20spaces.txt").as_deref(),
            Some("C:\\dir\\file with spaces.txt")
        );
        assert_eq!(
            file_url_to_path("file://srv/share/x").as_deref(),
            Some("\\\\srv\\share\\x")
        );
        assert_eq!(file_url_to_path("file:///tmp/x"), None);
        assert_eq!(file_url_to_path("file:///C:/a%2Fb"), None);
        assert_eq!(file_url_to_path("file:///C:/a%5Cb"), None);
    }
}
