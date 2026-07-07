#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    raw: String,
    alts: Vec<Alt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Alt {
    anchor_start: bool,
    anchor_end: bool,
    parts: Vec<String>,
}

impl Pattern {
    /// Compile a small, deterministic regex-compatible subset used by AegisHV config.
    ///
    /// Supported intentionally: alternation (`a|b`), `.*`, `^`, `$`, and case-insensitive
    /// prefix `(?i)`. Unsupported regex operators fail closed during config validation instead of
    /// being treated as a broader match.
    pub fn compile(input: &str) -> Result<Self, String> {
        let mut raw = input.trim().to_string();
        if raw.is_empty() {
            return Err("empty pattern".to_string());
        }
        let mut case_insensitive = false;
        if let Some(rest) = raw.strip_prefix("(?i)") {
            case_insensitive = true;
            raw = rest.to_string();
        }
        validate_balanced(&raw)?;
        reject_unsupported(&raw)?;
        let mut alts = Vec::new();
        for alt in raw.split('|') {
            let alt = alt.trim();
            if alt.is_empty() {
                return Err(format!("invalid pattern '{}': empty alternation", input));
            }
            let (anchor_start, body) = if let Some(v) = alt.strip_prefix('^') {
                (true, v)
            } else {
                (false, alt)
            };
            let (anchor_end, body) = if let Some(v) = body.strip_suffix('$') {
                (true, v)
            } else {
                (false, body)
            };
            let mut parts = Vec::new();
            for part in body.split(".*") {
                let mut p = unescape_regex_literal(part)?;
                if case_insensitive {
                    p = p.to_ascii_lowercase();
                }
                if !p.is_empty() {
                    parts.push(p);
                }
            }
            if parts.is_empty() {
                return Err(format!("invalid pattern '{}': empty expression", input));
            }
            alts.push(Alt {
                anchor_start,
                anchor_end,
                parts,
            });
        }
        Ok(Self {
            raw: input.to_string(),
            alts,
        })
    }

    pub fn is_match(&self, value: &str) -> bool {
        let mut hay = value.to_string();
        if self.raw.starts_with("(?i)") {
            hay = hay.to_ascii_lowercase();
        }
        self.alts.iter().any(|alt| alt_matches(alt, &hay))
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }
}

fn alt_matches(alt: &Alt, hay: &str) -> bool {
    if alt.parts.len() == 1 {
        let needle = &alt.parts[0];
        return match (alt.anchor_start, alt.anchor_end) {
            (true, true) => hay == needle,
            (true, false) => hay.starts_with(needle),
            (false, true) => hay.ends_with(needle),
            (false, false) => hay.contains(needle),
        };
    }

    let mut pos = 0usize;
    for (idx, part) in alt.parts.iter().enumerate() {
        if idx == 0 && alt.anchor_start {
            if !hay[pos..].starts_with(part) {
                return false;
            }
            pos += part.len();
            continue;
        }
        let Some(found) = hay[pos..].find(part) else {
            return false;
        };
        pos += found + part.len();
    }
    if alt.anchor_end {
        hay.ends_with(alt.parts.last().expect("nonempty"))
    } else {
        true
    }
}

fn validate_balanced(s: &str) -> Result<(), String> {
    let mut paren = 0i32;
    let mut bracket = 0i32;
    let mut escape = false;
    for c in s.chars() {
        if escape {
            escape = false;
            continue;
        }
        match c {
            '\\' => escape = true,
            '(' => paren += 1,
            ')' => {
                paren -= 1;
                if paren < 0 {
                    return Err("invalid pattern: unmatched ')'".to_string());
                }
            }
            '[' => bracket += 1,
            ']' => {
                bracket -= 1;
                if bracket < 0 {
                    return Err("invalid pattern: unmatched ']'".to_string());
                }
            }
            _ => {}
        }
    }
    if escape {
        return Err("invalid pattern: trailing escape".to_string());
    }
    if paren != 0 {
        return Err("invalid pattern: unmatched '('".to_string());
    }
    if bracket != 0 {
        return Err("invalid pattern: unmatched '['".to_string());
    }
    Ok(())
}

fn reject_unsupported(s: &str) -> Result<(), String> {
    let mut escape = false;
    for c in s.chars() {
        if escape {
            escape = false;
            continue;
        }
        if c == '\\' {
            escape = true;
            continue;
        }
        if matches!(c, '+' | '?' | '{' | '}' | '[' | ']' | '(' | ')') {
            return Err(format!(
                "invalid pattern: unsupported regex operator '{}'",
                c
            ));
        }
    }
    Ok(())
}

fn unescape_regex_literal(s: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut escape = false;
    for c in s.chars() {
        if escape {
            out.push(c);
            escape = false;
        } else if c == '\\' {
            escape = true;
        } else {
            out.push(c);
        }
    }
    if escape {
        return Err("invalid pattern: trailing escape".to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::Pattern;

    #[test]
    fn supports_alternation() {
        let p = Pattern::compile("VIOLATION|NPF|STAGE2").unwrap();
        assert!(p.is_match("EPT_VIOLATION"));
        assert!(p.is_match("NPF"));
        assert!(!p.is_match("IO"));
    }

    #[test]
    fn rejects_unbalanced_regex() {
        assert!(Pattern::compile("(").is_err());
    }
}
