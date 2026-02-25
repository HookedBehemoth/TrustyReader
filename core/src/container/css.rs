use core::ops::Add;

use alloc::{
    string::String,
    vec::Vec,
};

use crate::layout;

pub struct Stylesheet {
    rules: Vec<(Selector, Rule)>,
}

#[derive(Clone)]
struct Selector {
    element: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
}

impl Selector {
    /// Parse a single simple or compound selector such as `p`, `.intro`,
    /// `#main`, `p.intro`, or `h1#title.highlight`.
    ///
    /// Returns `None` for selectors that contain combinators (whitespace,
    /// `>`, `+`, `~`), pseudo-classes/elements, or attribute selectors –
    /// those are intentionally ignored.
    fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty()
            || s.contains(|c: char| {
                c.is_whitespace() || c == '>' || c == '+' || c == '~' || c == ':' || c == '['
            })
        {
            return None;
        }

        let mut element = None;
        let mut id = None;
        let mut classes = Vec::new();

        let mut current = String::new();
        let mut kind = 'e'; // 'e' = element, '.' = class, '#' = id

        for ch in s.chars() {
            match ch {
                '.' | '#' => {
                    let part = core::mem::take(&mut current);
                    if !part.is_empty() {
                        match kind {
                            'e' => element = Some(part),
                            '.' => classes.push(part),
                            '#' => id = Some(part),
                            _ => {}
                        }
                    }
                    kind = ch;
                }
                _ => current.push(ch),
            }
        }

        if !current.is_empty() {
            match kind {
                'e' => element = Some(current),
                '.' => classes.push(current),
                '#' => id = Some(current),
                _ => {}
            }
        }

        if element.is_none() && id.is_none() && classes.is_empty() {
            return None;
        }

        Some(Self {
            element,
            id,
            classes,
        })
    }

    fn matches(&self, element: &str, id: Option<&str>, classes: &[&str]) -> bool {
        if let Some(ref el) = self.element {
            if el != element {
                return false;
            }
        }
        if let Some(ref sel_id) = self.id {
            match id {
                Some(el_id) if el_id == sel_id.as_str() => {}
                _ => return false,
            }
        }
        self.classes.iter().all(|c| classes.contains(&c.as_str()))
    }

    /// Specificity as `(ids, classes, elements)`.
    fn specificity(&self) -> (u8, u8, u8) {
        (
            self.id.is_some() as u8,
            self.classes.len() as u8,
            self.element.is_some() as u8,
        )
    }
}

impl Stylesheet {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Look up the cascaded rule for an element given its tag name, optional
    /// `id` attribute, and optional `class` attribute (space-separated list).
    pub fn get(&self, element: &str, id: Option<&str>, class: Option<&str>) -> Rule {
        let classes: Vec<&str> = class
            .map(|c| c.split_whitespace().collect())
            .unwrap_or_default();

        let mut matches: Vec<((u8, u8, u8), usize, &Rule)> = self
            .rules
            .iter()
            .enumerate()
            .filter(|(_, (sel, _))| sel.matches(element, id, &classes))
            .map(|(i, (sel, rule))| (sel.specificity(), i, rule))
            .collect();

        // Lower specificity / earlier source order applied first so that
        // higher-specificity rules override.
        matches.sort_by_key(|&(spec, idx, _)| (spec, idx));

        matches
            .into_iter()
            .fold(Rule::default(), |acc, (_, _, rule)| acc + *rule)
    }

    pub fn extend_from_sheet(&mut self, sheet: &str) {
        let sheet = Self::filter_comments(sheet);

        let mut pos = 0;
        while pos < sheet.len() {
            let remaining = &sheet[pos..];
            let Some(brace_pos) = remaining.find(|c| c == '{' || c == '@') else {
                break;
            };

            let actual_pos = pos + brace_pos;

            // Skip at-rules
            if sheet.as_bytes()[actual_pos] == b'@' {
                if let Some(end) = Self::skip_at_rule(&sheet, actual_pos) {
                    pos = end;
                    continue;
                }
                break;
            }

            // Find matching closing brace (handles nested blocks)
            let Some(end_pos) = Self::find_closing_brace(&sheet, actual_pos) else {
                break;
            };

            let selector_text = sheet[pos..actual_pos].trim();
            let declarations = sheet[actual_pos + 1..end_pos].trim();

            // Ignore rules whose body contains nested braces (nested rules).
            if !declarations.contains('{') {
                let rule = Rule::from_str(declarations);
                if rule.has_any() {
                    // Handle grouped selectors (comma-separated).
                    for sel_str in selector_text.split(',') {
                        if let Some(selector) = Selector::parse(sel_str) {
                            self.rules.push((selector, rule));
                        }
                    }
                }
            }

            pos = end_pos + 1;
        }
    }

    fn skip_at_rule(sheet: &str, at_pos: usize) -> Option<usize> {
        let rest = &sheet[at_pos..];
        let semi = rest.find(';');
        let brace = rest.find('{');

        match (semi, brace) {
            (Some(s), Some(b)) if s < b => Some(at_pos + s + 1),
            (_, Some(b)) => Self::find_closing_brace(sheet, at_pos + b).map(|end| end + 1),
            (Some(s), None) => Some(at_pos + s + 1),
            (None, None) => None,
        }
    }

    fn find_closing_brace(sheet: &str, open_pos: usize) -> Option<usize> {
        let mut depth: u32 = 1;
        for (i, b) in sheet[open_pos + 1..].bytes().enumerate() {
            match b {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(open_pos + 1 + i);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn filter_comments(sheet: &str) -> String {
        let mut result = String::new();
        let mut chars = sheet.char_indices().peekable();

        while let Some((_, c)) = chars.next() {
            if c == '/' {
                if let Some((_, '*')) = chars.peek() {
                    chars.next(); // consume '*'
                    // Skip until we find */
                    let mut found_end = false;
                    while let Some((_, c)) = chars.next() {
                        if c == '*' {
                            if let Some((_, '/')) = chars.peek() {
                                chars.next(); // consume '/'
                                found_end = true;
                                break;
                            }
                        }
                    }
                    if !found_end {
                        break; // Unclosed comment
                    }
                } else {
                    result.push(c);
                }
            } else {
                result.push(c);
            }
        }

        result
    }
}

#[derive(Clone, Copy)]
pub struct Rule {
    pub alignment: Option<layout::Alignment>,
    pub italic: Option<bool>,
    pub bold: Option<bool>,
    pub indent: Option<u16>,
}

impl Rule {
    /// CSS from inside a "style" attribute or inside a stylesheet rule.
    pub fn from_str(s: &str) -> Self {
        let s = s.trim().to_ascii_lowercase();
        let parts = s.split(';').map(|p| p.trim());
        let mut rule = Self::default();
        for part in parts {
            let Some((key, value)) = part.split_once(':') else {
                continue;
            };

            match key.trim() {
                "text-align" => {
                    rule.alignment = match value.trim() {
                        "start" | "left" => Some(layout::Alignment::Start),
                        "end" | "right" => Some(layout::Alignment::End),
                        "center" => Some(layout::Alignment::Center),
                        "justify" => Some(layout::Alignment::Justify),
                        _ => None,
                    }
                }
                "font-style" => {
                    rule.italic = match value.trim() {
                        "normal" => Some(false),
                        "italic" => Some(true),
                        _ => None,
                    }
                }
                "font-weight" => {
                    rule.bold = match value.trim() {
                        "normal" => Some(false),
                        "bold" => Some(true),
                        _ => None,
                    }
                }
                "text-indent" => {
                    if let Some(indent) = value.trim().strip_suffix("px") {
                        if let Ok(indent) = indent.parse::<u16>() {
                            rule.indent = Some(indent);
                        }
                    }
                }
                _ => {}
            }
        }

        rule
    }

    fn has_any(&self) -> bool {
        self.alignment.is_some()
            || self.italic.is_some()
            || self.bold.is_some()
            || self.indent.is_some()
    }
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            alignment: None,
            italic: None,
            bold: None,
            indent: None,
        }
    }
}

impl Add for Rule {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            alignment: self.alignment.or(rhs.alignment),
            italic: self.italic.or(rhs.italic),
            bold: self.bold.or(rhs.bold),
            indent: self.indent.or(rhs.indent),
        }
    }
}
