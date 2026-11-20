//! Placement of captured text into a note (QuickAdd Capture rules).

#[derive(Debug, Clone, Default)]
pub struct InsertAfter {
    pub enabled: bool,
    pub after: String,
    pub insert_at_end: bool,
    pub consider_subsections: bool,
    pub create_if_not_found: bool,
    pub create_at_top: bool,
}

fn frontmatter_end(content: &str) -> usize {
    if !(content.starts_with("---\n") || content.starts_with("---\r\n")) {
        return 0;
    }
    let mut pos = content.find('\n').map(|i| i + 1).unwrap_or(0);
    for line in content[pos..].split_inclusive('\n') {
        pos += line.len();
        if line.trim_end() == "---" {
            return pos;
        }
    }
    0
}

fn heading_level(line: &str) -> usize {
    let n = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&n) && line[n..].starts_with(char::is_whitespace) {
        n
    } else {
        0
    }
}

fn section_end(lines: &[String], i: usize, consider_sub: bool) -> usize {
    let level = heading_level(&lines[i]);
    let mut fence = false;
    for (j, l) in lines.iter().enumerate().skip(i + 1) {
        let t = l.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        let h = if fence { 0 } else { heading_level(l) };
        if h > 0 && (!consider_sub || level == 0 || h <= level) {
            return j;
        }
    }
    lines.len()
}

fn with_trailing_newline(content: &str) -> String {
    if content.is_empty() || content.ends_with('\n') {
        content.to_string()
    } else {
        format!("{content}\n")
    }
}

pub fn apply_capture(
    content: &str,
    text: &str,
    prepend: bool,
    ia: &InsertAfter,
) -> Result<String, String> {
    if ia.enabled && !ia.after.trim().is_empty() {
        let target = ia.after.trim();
        let mut lines: Vec<String> = content.split('\n').map(String::from).collect();
        if let Some(i) = lines.iter().position(|l| l.trim() == target) {
            let at = if ia.insert_at_end {
                let mut at = section_end(&lines, i, ia.consider_subsections);
                while at > i + 1 && lines[at - 1].trim().is_empty() {
                    at -= 1;
                }
                at
            } else {
                let mut at = i + 1;
                if at < lines.len().saturating_sub(1) && lines[at].trim().is_empty() {
                    at += 1;
                }
                at
            };
            let new: Vec<String> = text.split('\n').map(String::from).collect();
            lines.splice(at..at, new);
            return Ok(lines.join("\n"));
        }
        if !ia.create_if_not_found {
            return Err(format!("Heading \"{}\" not found", ia.after));
        }
        if ia.create_at_top {
            let p = frontmatter_end(content);
            return Ok(format!("{}{target}\n{text}\n{}", &content[..p], &content[p..]));
        }
        return Ok(format!("{}\n{target}\n{text}\n", with_trailing_newline(content)));
    }
    if prepend {
        let p = frontmatter_end(content);
        return Ok(format!("{}{text}\n{}", &content[..p], &content[p..]));
    }
    Ok(format!("{}{text}\n", with_trailing_newline(content)))
}
