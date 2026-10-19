//! The file container of `Kittehface.Framework20.UserData` (`sav`, `sav2`, `sav3`): a
//! `Hashtable` of string keys to string values, serialised by `NetDataContractSerializer`
//! as UTF-8 XML with a byte-order mark and no declaration. The game writes
//! `MemoryStream.GetBuffer()`, so its files end in NUL padding, which reading drops
//! (`TruncateNullBytes`). Format: `rainworld-re/notes/saves.md`.
//!
//! Mirrors: -
//! (Kittehface.Framework20.UserData lives outside Assembly-CSharp)

use std::fmt;

const BOM: &str = "\u{feff}";
const ARRAYS_NS: &str = "http://schemas.microsoft.com/2003/10/Serialization/Arrays";
/// `HashHelpers.primes`' start, for `Hashtable.expand`.
const PRIMES: [usize; 23] = [
    3, 7, 11, 17, 23, 29, 37, 47, 59, 71, 89, 107, 131, 163, 197, 239, 293, 353, 431, 521, 631,
    761, 919,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDataError(pub String);

impl UserDataError {
    fn missing(element: &str) -> Self {
        Self(format!("no {element} element"))
    }
}

impl fmt::Display for UserDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UserDataError {}

/// One `Hashtable` entry. `same_as`: the value is the same object as an earlier entry's
/// (written as a `z:Ref`), as `BackUpSave` leaves it until the file is read again.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    key: String,
    value: String,
    same_as: Option<usize>,
}

/// A `UserData.File`'s data, in its serialised entry order.
#[derive(Debug, Clone, PartialEq)]
pub struct UserDataFile {
    load_factor: String,
    version: i32,
    hash_size: usize,
    entries: Vec<Entry>,
}

/// One `<anyType>` item as read: its `z:Id` and string, or a `z:Ref` to an earlier id.
enum Item {
    Text(Option<u32>, String),
    Ref(u32),
}

impl Default for UserDataFile {
    fn default() -> Self {
        Self::new()
    }
}

impl UserDataFile {
    /// `new Hashtable()`: no entries, three buckets.
    #[must_use]
    pub fn new() -> Self {
        Self {
            load_factor: "0.72".to_owned(),
            version: 0,
            hash_size: 3,
            entries: Vec::new(),
        }
    }

    /// Reads a file's bytes.
    ///
    /// # Errors
    /// When the bytes are not a serialised string `Hashtable`.
    pub fn parse(bytes: &[u8]) -> Result<Self, UserDataError> {
        let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        let text = std::str::from_utf8(&bytes[..end])
            .map_err(|e| UserDataError(format!("not UTF-8: {e}")))?;
        let text = text.strip_prefix(BOM).unwrap_or(text);
        if !text.starts_with("<ArrayOfKeyValueOfanyTypeanyType") {
            return Err(UserDataError("not a serialised Hashtable".to_owned()));
        }
        let load_factor = Self::element_text(text, "LoadFactor")?.to_owned();
        let version = Self::element_text(text, "Version")?
            .parse()
            .map_err(|e| UserDataError(format!("Version: {e}")))?;
        let hash_size = Self::element_text(text, "HashSize")?
            .parse()
            .map_err(|e| UserDataError(format!("HashSize: {e}")))?;
        let keys = Self::items(Self::element_text(text, "Keys")?)?;
        let values = Self::items(Self::element_text(text, "Values")?)?;
        if keys.len() != values.len() {
            return Err(UserDataError("keys and values differ in count".to_owned()));
        }
        // Resolve ids: keys first, then values, in document order.
        let mut ids: Vec<(u32, Option<usize>, String)> = Vec::new();
        let mut key_strings = Vec::new();
        for item in keys {
            match item {
                Item::Text(id, s) => {
                    if let Some(id) = id {
                        ids.push((id, None, s.clone()));
                    }
                    key_strings.push(s);
                }
                Item::Ref(id) => key_strings.push(Self::resolve(&ids, id)?.1),
            }
        }
        let mut entries: Vec<Entry> = Vec::new();
        for (i, (item, key)) in values.into_iter().zip(key_strings).enumerate() {
            let entry = match item {
                Item::Text(id, value) => {
                    if let Some(id) = id {
                        ids.push((id, Some(i), value.clone()));
                    }
                    Entry {
                        key,
                        value,
                        same_as: None,
                    }
                }
                Item::Ref(id) => {
                    let (same_as, value) = Self::resolve(&ids, id)?;
                    Entry {
                        key,
                        value,
                        same_as,
                    }
                }
            };
            entries.push(entry);
        }
        Ok(Self {
            load_factor,
            version,
            hash_size,
            entries,
        })
    }

    fn resolve(
        ids: &[(u32, Option<usize>, String)],
        id: u32,
    ) -> Result<(Option<usize>, String), UserDataError> {
        ids.iter()
            .find(|(i, _, _)| *i == id)
            .map(|(_, entry, s)| (*entry, s.clone()))
            .ok_or_else(|| UserDataError(format!("z:Ref {id} to no object")))
    }

    /// The content of the first `<name ...>...</name>` element.
    fn element_text<'a>(text: &'a str, name: &str) -> Result<&'a str, UserDataError> {
        let start = text
            .find(&format!("<{name} "))
            .ok_or_else(|| UserDataError::missing(name))?;
        let open_end = start
            + text[start..]
                .find('>')
                .ok_or_else(|| UserDataError::missing(name))?
            + 1;
        let close = open_end
            + text[open_end..]
                .find(&format!("</{name}>"))
                .ok_or_else(|| UserDataError::missing(name))?;
        Ok(&text[open_end..close])
    }

    fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
        let at = tag.find(&format!(" {name}=\""))? + name.len() + 3;
        let len = tag[at..].find('"')?;
        Some(&tag[at..at + len])
    }

    /// The `<anyType>` items of an array element's content.
    fn items(mut text: &str) -> Result<Vec<Item>, UserDataError> {
        let mut items = Vec::new();
        while let Some(start) = text.find("<anyType") {
            let tag_end = start
                + text[start..]
                    .find('>')
                    .ok_or_else(|| UserDataError("unclosed anyType".to_owned()))?;
            let tag = &text[start..tag_end];
            let id = Self::attribute(tag, "z:Id").and_then(|v| v.parse().ok());
            if tag.ends_with('/') {
                match Self::attribute(tag, "z:Ref") {
                    Some(r) => items.push(Item::Ref(
                        r.parse()
                            .map_err(|e| UserDataError(format!("z:Ref: {e}")))?,
                    )),
                    None => items.push(Item::Text(id, String::new())),
                }
                text = &text[tag_end + 1..];
                continue;
            }
            let content = &text[tag_end + 1..];
            let close = content
                .find("</anyType>")
                .ok_or_else(|| UserDataError("unclosed anyType".to_owned()))?;
            items.push(Item::Text(id, Self::unescape(&content[..close])));
            text = &content[close + "</anyType>".len()..];
        }
        Ok(items)
    }

    fn unescape(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(amp) = rest.find('&') {
            out.push_str(&rest[..amp]);
            let after = &rest[amp..];
            let Some(semi) = after.find(';') else {
                out.push_str(after);
                return out;
            };
            let entity = &after[1..semi];
            let decoded = match entity {
                "lt" => Some('<'),
                "gt" => Some('>'),
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => entity
                    .strip_prefix("#x")
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            match decoded {
                Some(c) => out.push(c),
                None => out.push_str(&after[..=semi]),
            }
            rest = &after[semi + 1..];
        }
        out.push_str(rest);
        out
    }

    fn escape(s: &str, out: &mut String) {
        for c in s.chars() {
            match c {
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '&' => out.push_str("&amp;"),
                '\r' => out.push_str("&#xD;"),
                _ => out.push(c),
            }
        }
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.key == key)
            .map(|e| e.value.as_str())
    }

    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|e| e.key == key)
    }

    #[must_use]
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    /// `Hashtable.Insert`: expands first when full, then sets or adds (a new key goes
    /// last; the game's order follows the string hashes: docs/divergences/).
    pub fn set(&mut self, key: &str, value: String) {
        self.insert(key, value, None);
    }

    /// `BackUpSave`: `key` gets the very value object of `from`, if there is one.
    pub fn set_same_as(&mut self, key: &str, from: &str) {
        let Some(i) = self.entries.iter().position(|e| e.key == from) else {
            return;
        };
        let source = self.entries[i].same_as.unwrap_or(i);
        let value = self.entries[source].value.clone();
        self.insert(key, value, Some(source));
    }

    fn insert(&mut self, key: &str, value: String, same_as: Option<usize>) {
        let load_size =
            (self.load_factor.parse::<f32>().unwrap_or(0.72) * self.hash_size as f32) as usize;
        if self.entries.len() >= load_size {
            self.hash_size = PRIMES
                .iter()
                .copied()
                .find(|&p| p >= 2 * self.hash_size)
                .unwrap_or(2 * self.hash_size + 1);
            self.version = self.version.wrapping_add(1);
        }
        self.version = self.version.wrapping_add(1);
        match self.entries.iter().position(|e| e.key == key) {
            Some(i) => {
                // Entries sharing the old object keep it.
                let old = self.entries[i].value.clone();
                for e in &mut self.entries {
                    if e.same_as == Some(i) {
                        e.value.clone_from(&old);
                        e.same_as = None;
                    }
                }
                self.entries[i].value = value;
                self.entries[i].same_as = same_as.filter(|&s| s < i);
                // A reference points back: a later source becomes the reference.
                if let Some(s) = same_as.filter(|&s| s > i) {
                    self.entries[s].same_as = Some(i);
                }
            }
            None => self.entries.push(Entry {
                key: key.to_owned(),
                value,
                same_as,
            }),
        }
    }

    /// The serialised bytes (no NUL padding).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let n = self.entries.len();
        let mut s = String::with_capacity(
            self.entries
                .iter()
                .map(|e| e.key.len() + e.value.len())
                .sum::<usize>()
                + 1024,
        );
        s.push_str(BOM);
        s.push_str(
            "<ArrayOfKeyValueOfanyTypeanyType xmlns:i=\"http://www.w3.org/2001/XMLSchema-instance\" \
             xmlns:x=\"http://www.w3.org/2001/XMLSchema\" z:Id=\"1\" z:Type=\"System.Collections.Hashtable\" \
             z:Assembly=\"0\" xmlns:z=\"http://schemas.microsoft.com/2003/10/Serialization/\" \
             xmlns=\"http://schemas.microsoft.com/2003/10/Serialization/Arrays\">",
        );
        s.push_str(&format!(
            "<LoadFactor z:Id=\"2\" z:Type=\"System.Single\" z:Assembly=\"0\" xmlns=\"\">{}</LoadFactor>\
             <Version z:Id=\"3\" z:Type=\"System.Int32\" z:Assembly=\"0\" xmlns=\"\">{}</Version>\
             <Comparer i:nil=\"true\" xmlns=\"\" /><HashCodeProvider i:nil=\"true\" xmlns=\"\" />\
             <HashSize z:Id=\"4\" z:Type=\"System.Int32\" z:Assembly=\"0\" xmlns=\"\">{}</HashSize>\
             <Keys z:Id=\"5\" z:Type=\"System.Object[]\" z:Assembly=\"0\" z:Size=\"{n}\" xmlns=\"\">",
            self.load_factor, self.version, self.hash_size
        ));
        let mut next_id = 6;
        for e in &self.entries {
            Self::push_item(&mut s, next_id, &e.key);
            next_id += 1;
        }
        s.push_str(&format!(
            "</Keys><Values z:Id=\"{next_id}\" z:Type=\"System.Object[]\" z:Assembly=\"0\" z:Size=\"{n}\" xmlns=\"\">"
        ));
        next_id += 1;
        let mut value_ids = Vec::with_capacity(n);
        for e in &self.entries {
            match e.same_as.and_then(|i| value_ids.get(i).copied()) {
                Some(id) => {
                    s.push_str(&format!(
                        "<anyType i:type=\"x:string\" z:Ref=\"{id}\" i:nil=\"true\" xmlns=\"{ARRAYS_NS}\" />"
                    ));
                    value_ids.push(id);
                }
                None => {
                    Self::push_item(&mut s, next_id, &e.value);
                    value_ids.push(next_id);
                    next_id += 1;
                }
            }
        }
        s.push_str("</Values></ArrayOfKeyValueOfanyTypeanyType>");
        s.into_bytes()
    }

    fn push_item(s: &mut String, id: u32, text: &str) {
        s.push_str(&format!(
            "<anyType i:type=\"x:string\" z:Id=\"{id}\" xmlns=\"{ARRAYS_NS}\">"
        ));
        Self::escape(text, s);
        s.push_str("</anyType>");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_padding_and_ref() {
        let mut file = UserDataFile::new();
        file.set("save", "abc<progDivA>x&y".to_owned());
        file.set_same_as("save__Backup", "save");
        let bytes = file.to_bytes();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.contains("abc&lt;progDivA&gt;x&amp;y"));
        assert!(text.contains("z:Ref=\"9\" i:nil=\"true\""));
        let mut padded = bytes.clone();
        padded.extend([0; 100]);
        let read = UserDataFile::parse(&padded).unwrap();
        assert_eq!(read.get("save__Backup"), Some("abc<progDivA>x&y"));
        assert_eq!(read.to_bytes(), bytes);
        assert_eq!(read, file);
    }

    #[test]
    fn hashtable_growth() {
        let mut file = UserDataFile::new();
        file.set("a", String::new());
        file.set("b", String::new());
        assert_eq!((file.hash_size, file.version), (3, 2));
        file.set("c", String::new());
        assert_eq!((file.hash_size, file.version), (7, 4));
        file.set("a", "1".to_owned());
        assert_eq!(file.keys(), ["a", "b", "c"]);
        assert_eq!(file.get("a"), Some("1"));
    }
}
