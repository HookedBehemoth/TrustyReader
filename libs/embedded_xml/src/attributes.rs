/// XML attribute reader
#[derive(Clone)]
pub struct AttributeReader<'a> {
    remaining: &'a str,
}

impl core::fmt::Debug for AttributeReader<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut builder = f.debug_map();
        for (n, v) in self.clone() {
            builder.entry(&n, &v);
        }
        builder.finish()
    }
}

impl Default for AttributeReader<'_> {
    fn default() -> Self {
        AttributeReader { remaining: "" }
    }
}

impl PartialEq for AttributeReader<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.clone()
            .zip(other.clone())
            .all(|((n1, v1), (n2, v2))| n1.eq_ignore_ascii_case(n2) && v1 == v2)
    }
}

impl<'a> AttributeReader<'a> {
    /// ```
    /// # use embedded_xml::AttributeReader;
    /// let mut reader = AttributeReader::from_block(r#"foo="bar" baz='qux'"#);
    /// assert_eq!(reader.next(), Some(("foo", "bar")));
    /// assert_eq!(reader.next(), Some(("baz", "qux")));
    /// assert_eq!(reader.next(), None);
    /// ```
    pub fn from_block(buffer: &str) -> AttributeReader<'_> {
        AttributeReader {
            remaining: buffer.trim_ascii(),
        }
    }

    /// Case-insensitive search by attribute name. Returns the value or None.
    /// ```
    /// # use embedded_xml::AttributeReader;
    /// let reader = AttributeReader::from_block(r#"foo="bar" baz='qux'"#);
    /// assert_eq!(reader.get("foo"), Some("bar"));
    /// assert_eq!(reader.get("baz"), Some("qux"));
    /// assert_eq!(reader.get("nonexistent"), None);
    /// assert_eq!(reader.get("foo"), Some("bar"));
    /// ```
    pub fn get(&self, name: &str) -> Option<&str> {
        for (n, v) in self.clone() {
            if n.eq_ignore_ascii_case(name) {
                return Some(v);
            }
        }
        None
    }
}

impl<'a> Iterator for AttributeReader<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let s = self.remaining.trim_ascii_start();
        if s.is_empty() {
            return None;
        }
        let eq_pos = s.find('=')?;
        let name = &s[..eq_pos];
        let after_eq = &s[eq_pos + 1..];
        let quote = *after_eq.as_bytes().first()?;
        if quote == b'"' || quote == b'\'' {
            let value_start = &after_eq[1..];
            let end_pos = value_start.find(quote as char)?;
            let value = &value_start[..end_pos];
            self.remaining = &value_start[end_pos + 1..];
            Some((name, value))
        } else {
            let end_pos = after_eq
                .find(|c: char| c.is_ascii_whitespace())
                .unwrap_or(after_eq.len());
            let value = &after_eq[..end_pos];
            self.remaining = &after_eq[end_pos..];
            Some((name, value))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let mut reader = AttributeReader::from_block(r#"foo="bar" baz='qux'"#);
        assert_eq!(reader.next(), Some(("foo", "bar")));
        assert_eq!(reader.next(), Some(("baz", "qux")));
        assert_eq!(reader.next(), None);
    }

    #[test]
    fn whitespace() {
        let mut reader = AttributeReader::from_block(r#"style="text-align: center""#);
        assert_eq!(reader.next(), Some(("style", "text-align: center")));
        assert_eq!(reader.next(), None);
    }
}
