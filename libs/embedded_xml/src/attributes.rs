/// XML attribute reader
#[derive(Clone)]
pub struct AttributeReader<'a> {
    split: core::str::SplitAsciiWhitespace<'a>,
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
        AttributeReader {
            split: "".split_ascii_whitespace(),
        }
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
            split: buffer.trim_ascii().split_ascii_whitespace(),
        }
    }

    /// ```
    /// # use embedded_xml::AttributeReader;
    /// let mut split = r#"item foo="bar" baz='qux'"#.split_ascii_whitespace();
    /// let name = split.next().unwrap();
    /// assert_eq!(name, "item");
    /// let mut reader = AttributeReader::from_split(split);
    /// assert_eq!(reader.next(), Some(("foo", "bar")));
    /// assert_eq!(reader.next(), Some(("baz", "qux")));
    /// assert_eq!(reader.next(), None);
    /// ```
    pub fn from_split(split: core::str::SplitAsciiWhitespace<'_>) -> AttributeReader<'_> {
        AttributeReader { split }
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
        let part = self.split.next()?;
        let mut iter = part.splitn(2, '=');
        let name = iter.next()?;
        let value = iter.next()?.trim_matches('"').trim_matches('\'');
        Some((name, value))
    }
}
