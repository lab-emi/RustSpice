//! Deck decoding and `.include` / `.lib` expansion.
//!
//! Included text is spliced in as physical lines, before comments and `+` continuations are processed, so an
//! included file keeps its own comments and continuations. Its lines are reported at the line of the top-level
//! card that brought them in. `.lib <file> <section>` splices only the `.lib <section>` ... `.endl` block of the
//! file; section definitions that nothing selects are skipped. Bounded: include depth, a cycle guard on
//! (file, section), and the total included bytes against `max_expanded_bytes`.

use std::path::{Path, PathBuf};

use crate::error::{SimError, SimResult};
use crate::limits::ResourceLimits;

/// Deepest `.include` / `.lib` nesting accepted.
pub const MAX_INCLUDE_DEPTH: usize = 16;

/// One physical input line and the main-deck line it is reported at.
#[derive(Debug)]
pub(crate) struct SourceLine {
    pub line: usize,
    pub text: String,
}

fn parse_err(line: usize, message: impl Into<String>) -> SimError {
    SimError::ParseError {
        line,
        message: message.into(),
    }
}

/// Decodes deck bytes. UTF-16 (byte-order mark) is refused; invalid UTF-8 becomes U+FFFD, which the reader accepts
/// only in the title and in comments. A NUL byte is an ordinary character (it ends a number like any other junk).
pub fn decode_deck(bytes: &[u8]) -> SimResult<String> {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return Err(parse_err(
            1,
            "the file is UTF-16 encoded (byte-order mark); save the deck as ASCII or UTF-8",
        ));
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

/// The file-handling cards.
enum Card {
    Include(String),
    LibSection(String, String),
    LibDef(String),
    Endl,
    Bad(String),
}

/// Arguments of a card: whitespace separated, `"..."` / `'...'` quoted, up to a `;` comment.
fn arguments(rest: &str) -> Vec<String> {
    let rest = rest.split(';').next().unwrap_or("");
    let mut out = Vec::new();
    let mut chars = rest.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '"' || c == '\'' {
            chars.next();
            out.push(chars.by_ref().take_while(|&d| d != c).collect());
        } else {
            let mut tok = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_whitespace() {
                    break;
                }
                tok.push(d);
                chars.next();
            }
            out.push(tok);
        }
    }
    out
}

fn classify(raw: &str) -> Option<Card> {
    let t = raw.trim_start();
    if !t.starts_with('.') {
        return None;
    }
    let head_len = t.find(char::is_whitespace).unwrap_or(t.len());
    let head = t[..head_len].to_ascii_lowercase();
    let args = arguments(&t[head_len..]);
    match head.as_str() {
        ".include" | ".inc" => Some(match args.as_slice() {
            [file] => Card::Include(file.clone()),
            _ => Card::Bad(format!("`{head}` needs exactly one file name")),
        }),
        ".lib" => Some(match args.as_slice() {
            [file, section] => Card::LibSection(file.clone(), section.clone()),
            [name] => Card::LibDef(name.clone()),
            _ => Card::Bad("`.lib` needs a file name and a section name".to_string()),
        }),
        ".endl" => Some(Card::Endl),
        _ => None,
    }
}

struct Expander<'a> {
    /// Files (and selected sections) being read, outermost first.
    stack: Vec<(PathBuf, Option<String>)>,
    bytes: usize,
    limits: &'a ResourceLimits,
}

impl Expander<'_> {
    /// Splices `text` into `out`. `origin` is the top-level line every line is reported at (`None` for the main
    /// deck, whose line 1 is the title); `section` selects one `.lib` block.
    fn splice(
        &mut self,
        text: &str,
        origin: Option<usize>,
        dir: &Path,
        section: Option<&str>,
        out: &mut Vec<SourceLine>,
    ) -> SimResult<()> {
        let mut in_control = false;
        let mut block: Option<(usize, String)> = None;
        let mut found = section.is_none();
        for (i, raw) in text.lines().enumerate() {
            let line = origin.unwrap_or(i + 1);
            if origin.is_none() && i == 0 {
                out.push(SourceLine {
                    line,
                    text: raw.to_string(),
                });
                continue;
            }
            let card = if in_control { None } else { classify(raw) };
            match &card {
                Some(Card::LibDef(name)) => {
                    if let Some((_, open)) = &block {
                        return Err(parse_err(
                            line,
                            format!("`.lib {name}` starts inside the section `{open}` (missing `.endl`)"),
                        ));
                    }
                    if section.is_some_and(|s| s.eq_ignore_ascii_case(name)) {
                        found = true;
                    }
                    block = Some((line, name.clone()));
                    continue;
                }
                Some(Card::Endl) => {
                    if block.take().is_none() {
                        return Err(parse_err(
                            line,
                            "`.endl` without a matching `.lib <section>`",
                        ));
                    }
                    continue;
                }
                _ => {}
            }
            let active = match (section, &block) {
                (None, None) => true,
                (Some(s), Some((_, b))) => s.eq_ignore_ascii_case(b),
                // a section nobody selected here, or text outside the sections of a library file
                _ => false,
            };
            if !active {
                continue;
            }
            let lower = raw.trim_start().to_ascii_lowercase();
            if lower.starts_with(".control") {
                in_control = true;
            } else if lower.starts_with(".endc") {
                in_control = false;
            }
            match card {
                Some(Card::Include(file)) => self.enter(&file, None, line, dir, out)?,
                Some(Card::LibSection(file, sec)) => {
                    self.enter(&file, Some(&sec), line, dir, out)?
                }
                Some(Card::Bad(msg)) => return Err(parse_err(line, msg)),
                _ => out.push(SourceLine {
                    line,
                    text: raw.to_string(),
                }),
            }
        }
        if let Some((start, name)) = block {
            return Err(parse_err(
                origin.unwrap_or(start),
                format!("`.lib {name}` has no matching `.endl` (to read a library use `.lib <file> <section>`)"),
            ));
        }
        if !found {
            let (file, _) = self.stack.last().cloned().unwrap_or_default();
            return Err(parse_err(
                origin.unwrap_or(0),
                format!(
                    "section `{}` is not defined in `{}`",
                    section.unwrap_or(""),
                    file.display()
                ),
            ));
        }
        Ok(())
    }

    fn enter(
        &mut self,
        file: &str,
        section: Option<&str>,
        line: usize,
        dir: &Path,
        out: &mut Vec<SourceLine>,
    ) -> SimResult<()> {
        // next to the including file first, then in the working directory
        let mut path = dir.join(file);
        if !path.exists() && Path::new(file).is_relative() {
            if let Ok(cwd) = std::env::current_dir() {
                let alt = cwd.join(file);
                if alt.exists() {
                    path = alt;
                }
            }
        }
        let canon = std::fs::canonicalize(&path).map_err(|e| {
            parse_err(
                line,
                format!(
                    "cannot read the included file `{file}` ({}): {e}",
                    path.display()
                ),
            )
        })?;
        let key = (canon.clone(), section.map(str::to_ascii_lowercase));
        if self.stack.contains(&key) {
            let chain: Vec<String> = self
                .stack
                .iter()
                .chain(std::iter::once(&key))
                .map(|(p, s)| match s {
                    Some(s) => format!("{} [{s}]", p.display()),
                    None => p.display().to_string(),
                })
                .collect();
            return Err(parse_err(
                line,
                format!("include cycle: {}", chain.join(" -> ")),
            ));
        }
        if self.stack.len() > MAX_INCLUDE_DEPTH {
            return Err(parse_err(
                line,
                format!("`.include` / `.lib` nesting deeper than {MAX_INCLUDE_DEPTH} levels"),
            ));
        }
        let size = std::fs::metadata(&canon).map(|m| m.len()).unwrap_or(0) as usize;
        let limit = self.limits.max_expanded_bytes;
        if self.bytes.saturating_add(size) > limit {
            return Err(SimError::ResourceLimit {
                resource: "included bytes",
                requested: format!("{} bytes", self.bytes.saturating_add(size)),
                limit: format!("{limit} bytes"),
                hint: "raise --max-expanded-mib if the included files are intended".to_string(),
            });
        }
        let bytes = std::fs::read(&canon)
            .map_err(|e| parse_err(line, format!("cannot read the included file `{file}`: {e}")))?;
        self.bytes += bytes.len();
        let text = decode_deck(&bytes).map_err(|e| match e {
            SimError::ParseError { message, .. } => {
                parse_err(line, format!("included file `{file}`: {message}"))
            }
            other => other,
        })?;
        let child_dir = canon.parent().map(Path::to_path_buf).unwrap_or_default();
        self.stack.push(key);
        let r = self.splice(&text, Some(line), &child_dir, section, out);
        self.stack.pop();
        r
    }
}

/// Physical lines of a deck with every `.include` / `.lib` spliced in. Relative file names resolve against the
/// directory of the including file (the deck's directory for `deck`, else the working directory), then against the
/// working directory.
pub(crate) fn expand(
    src: &str,
    deck: Option<&Path>,
    limits: &ResourceLimits,
) -> SimResult<Vec<SourceLine>> {
    let mut ex = Expander {
        stack: Vec::new(),
        bytes: 0,
        limits,
    };
    let dir = match deck {
        Some(p) => {
            if let Ok(c) = std::fs::canonicalize(p) {
                ex.stack.push((c, None));
            }
            p.parent().map(Path::to_path_buf).unwrap_or_default()
        }
        None => PathBuf::new(),
    };
    let mut out = Vec::new();
    ex.splice(src, None, &dir, None, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding_refuses_utf16_and_keeps_latin1_visible() {
        assert!(decode_deck(&[0xFF, 0xFE, b'a', 0]).is_err());
        assert_eq!(
            decode_deck(b"t\nR1 a 0 1\0k\n").unwrap(),
            "t\nR1 a 0 1\0k\n"
        );
        let s = decode_deck(b"t\xe9\n* caf\xe9\nR1 a 0 1\n").unwrap();
        assert!(s.contains('\u{FFFD}'));
        assert_eq!(decode_deck(b"\xEF\xBB\xBFtitle\n").unwrap(), "title\n");
    }

    #[test]
    fn arguments_honour_quotes_and_comments() {
        assert_eq!(arguments(r#" "a b.lib" tt ; x"#), vec!["a b.lib", "tt"]);
        assert_eq!(arguments(" 'f.lib'"), vec!["f.lib"]);
    }

    #[test]
    fn files_sections_cycles_and_missing_files() {
        let dir = std::env::temp_dir().join(format!("rustspice_inc_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("m.lib"), "* models\n.model DA D(IS=1e-12)\n").unwrap();
        std::fs::write(
            dir.join("c.lib"),
            ".lib tt\n.model DC D(IS=1)\n.endl\n.lib ff\n.model DC D(IS=2)\n.endl ff\n",
        )
        .unwrap();
        std::fs::write(dir.join("a.lib"), ".include b.lib\n").unwrap();
        std::fs::write(dir.join("b.lib"), ".inc 'a.lib'\n").unwrap();
        let deck = dir.join("deck.sp");
        let lim = ResourceLimits::default();
        let lines = expand(
            "t\n.include m.lib\n.lib c.lib FF\nR1 a 0 1\n",
            Some(&deck),
            &lim,
        )
        .unwrap();
        let text: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(
            text,
            vec![
                "t",
                "* models",
                ".model DA D(IS=1e-12)",
                ".model DC D(IS=2)",
                "R1 a 0 1"
            ]
        );
        assert_eq!(lines[2].line, 2);
        assert_eq!(lines[3].line, 3);
        let e = expand("t\n.include a.lib\n", Some(&deck), &lim)
            .unwrap_err()
            .to_string();
        assert!(e.contains("include cycle"), "{e}");
        let e = expand("t\n.include nope.lib\n", Some(&deck), &lim)
            .unwrap_err()
            .to_string();
        assert!(e.contains("nope.lib"), "{e}");
        let e = expand("t\n.lib c.lib SS\n", Some(&deck), &lim)
            .unwrap_err()
            .to_string();
        assert!(e.contains("section `SS`"), "{e}");
        let small = ResourceLimits {
            max_expanded_bytes: 4,
            ..ResourceLimits::default()
        };
        assert!(matches!(
            expand("t\n.include m.lib\n", Some(&deck), &small),
            Err(SimError::ResourceLimit { .. })
        ));
        std::fs::remove_dir_all(&dir).ok();
    }
}
