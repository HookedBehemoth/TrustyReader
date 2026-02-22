use core::ops::Add;
use alloc::{collections::btree_map::BTreeMap, string::{String, ToString}};

use crate::layout;


pub struct Stylesheet {
    rules: BTreeMap<String, Rule>,
}

impl Stylesheet {
    pub fn new() -> Self {
        Self {
            rules: BTreeMap::new(),
        }
    }

    pub fn get(&self, class: &str) -> Rule {
        class.split_whitespace()
            .filter_map(|c| self.rules.get(c))
            .fold(Rule::default(), |acc, rule| acc + rule.clone())
    }

    pub fn insert(&mut self, class: String, rule: Rule) {
        self.rules.insert(class, rule);
    }

    pub fn extend_from_sheet(&mut self, sheet: &str) {
        let sheet = Self::filter_comments(sheet);
        
        let mut pos = 0;
        while pos < sheet.len() {
            // Find next opening brace or @ symbol
            let remaining = &sheet[pos..];
            let Some(brace_pos) = remaining.find(|c| c == '{' || c == '@') else {
                break;
            };
            
            let actual_pos = pos + brace_pos;
            
            // Handle at-rules (skip them)
            if sheet.as_bytes()[actual_pos] == b'@' {
                // Find semicolon first
                if let Some(semi_pos) = sheet[actual_pos..].find(';') {
                    let semi_actual = actual_pos + semi_pos;
                    // Check if there's a brace before the semicolon
                    if let Some(brace) = sheet[actual_pos..semi_actual].find('{') {
                        // At-rule with block, skip to closing brace
                        if let Some(end_brace) = sheet[actual_pos + brace..].find('}') {
                            pos = actual_pos + brace + end_brace + 1;
                            continue;
                        }
                    } else {
                        // Simple at-rule, skip to semicolon
                        pos = semi_actual + 1;
                        continue;
                    }
                } else if let Some(brace) = sheet[actual_pos..].find('{') {
                    // At-rule with block but no semicolon
                    if let Some(end_brace) = sheet[actual_pos + brace..].find('}') {
                        pos = actual_pos + brace + end_brace + 1;
                        continue;
                    }
                }
                break;
            }
            
            // Extract selector
            let selector = sheet[pos..actual_pos].trim();
            
            // Find closing brace
            let Some(end_brace_pos) = sheet[actual_pos..].find('}') else {
                break;
            };
            let end_pos = actual_pos + end_brace_pos;
            
            // Extract declarations
            let declarations = sheet[actual_pos + 1..end_pos].trim();
            
            // Only handle class selectors (starting with '.')
            if let Some(class_name) = selector.strip_prefix('.') {
                let rule = Rule::from_str(declarations);
                // Only insert if the rule has at least one property
                if rule.alignment.is_some() || rule.italic.is_some() 
                    || rule.bold.is_some() || rule.indent.is_some() {
                    self.rules.insert(class_name.to_string(), rule);
                }
            }
            
            pos = end_pos + 1;
        }
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
                        "start" => Some(layout::Alignment::Start),
                        "end" => Some(layout::Alignment::End),
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
