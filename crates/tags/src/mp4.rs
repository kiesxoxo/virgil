//! iTunes metadata (MP4 `ilst`) as foobar2000 names it. Standard text atoms
//! get their usual names, `----:com.apple.iTunes:NAME` atoms go by NAME, and
//! track/disc numbers come from `trkn`/`disk`. Pictures and other atoms are
//! left alone.

use std::borrow::Cow;

use anyhow::Context;
use lofty::mp4::{Atom, AtomData, AtomIdent, Ilst};
use lofty::tag::Accessor;
use virgil_core::TagField;

use crate::{Native, push};

const TEXT: [(&[u8; 4], &str); 18] = [
    (b"\xa9nam", "TITLE"),
    (b"\xa9ART", "ARTIST"),
    (b"\xa9alb", "ALBUM"),
    (b"aART", "ALBUM ARTIST"),
    (b"\xa9day", "DATE"),
    (b"\xa9gen", "GENRE"),
    (b"\xa9wrt", "COMPOSER"),
    (b"\xa9cmt", "COMMENT"),
    (b"\xa9lyr", "LYRICS"),
    (b"\xa9grp", "GROUPING"),
    (b"cprt", "COPYRIGHT"),
    (b"\xa9too", "ENCODED BY"),
    (b"desc", "DESCRIPTION"),
    (b"soal", "ALBUMSORTORDER"),
    (b"soar", "ARTISTSORTORDER"),
    (b"sonm", "TITLESORTORDER"),
    (b"soaa", "ALBUMARTISTSORTORDER"),
    (b"soco", "COMPOSERSORTORDER"),
];

const ITUNES: &str = "com.apple.iTunes";
const NUMBERS: [&str; 4] = ["TRACKNUMBER", "TOTALTRACKS", "DISCNUMBER", "TOTALDISCS"];

fn text_values<'a>(atom: &'a Atom<'_>) -> Option<Vec<&'a str>> {
    atom.data()
        .map(|data| match data {
            AtomData::UTF8(s) | AtomData::UTF16(s) => Some(s.as_str()),
            _ => None,
        })
        .collect()
}

/// The field name Virgil shows for this atom, or None to leave it alone.
fn name_of(atom: &Atom<'_>) -> Option<String> {
    match atom.ident() {
        AtomIdent::Fourcc(cc) if cc == b"trkn" || cc == b"disk" => Some(String::new()),
        AtomIdent::Fourcc(cc) => TEXT.iter().find(|(c, _)| *c == cc).map(|(_, name)| name.to_string()),
        AtomIdent::Freeform { mean, name } if mean == ITUNES && text_values(atom).is_some() => Some(name.to_string()),
        AtomIdent::Freeform { .. } => None,
    }
}

impl Native for Ilst {
    fn fields(&self) -> Vec<TagField> {
        let mut fields = Vec::new();
        for atom in self {
            match name_of(atom) {
                Some(name) if !name.is_empty() => push(&mut fields, &name, text_values(atom).unwrap_or_default()),
                _ => {}
            }
        }
        let numbers = [self.track(), self.track_total(), self.disk(), self.disk_total()];
        for (name, number) in NUMBERS.iter().zip(numbers) {
            if let Some(n) = number.filter(|n| *n > 0) {
                push(&mut fields, name, [n.to_string()]);
            }
        }
        fields
    }

    fn replace(&mut self, fields: &[TagField]) -> anyhow::Result<()> {
        self.retain(|atom| name_of(atom).is_none());
        for field in fields {
            let name = field.name.as_str();
            if let Some(i) = NUMBERS.iter().position(|n| n.eq_ignore_ascii_case(name)) {
                let value = &field.values[0];
                let n: u32 = value
                    .trim()
                    .parse()
                    .ok()
                    .with_context(|| format!("{name} must be a number in MP4 files, not \"{value}\""))?;
                match i {
                    0 => self.set_track(n),
                    1 => self.set_track_total(n),
                    2 => self.set_disk(n),
                    _ => self.set_disk_total(n),
                }
                continue;
            }
            let ident = match TEXT.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)) {
                Some((cc, _)) => AtomIdent::Fourcc(**cc),
                None => AtomIdent::Freeform { mean: Cow::Borrowed(ITUNES), name: Cow::Owned(name.to_string()) },
            };
            let data = field.values.iter().map(|v| AtomData::UTF8(v.clone())).collect();
            self.insert(Atom::from_collection(ident, data).context("a field needs a value")?);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str, values: &[&str]) -> TagField {
        TagField { name: name.into(), values: values.iter().map(|v| v.to_string()).collect() }
    }

    #[test]
    fn maps_atoms_to_foobar_names_and_back() {
        let fields = vec![
            field("TITLE", &["Conta"]),
            field("ARTIST", &["A", "B"]),
            field("ALBUM ARTIST", &["Various"]),
            field("MOOD", &["calm"]),
            field("TRACKNUMBER", &["3"]),
            field("TOTALTRACKS", &["12"]),
        ];
        let mut ilst = Ilst::default();
        ilst.replace(&fields).unwrap();
        assert_eq!(ilst.track(), Some(3));
        let mut read = ilst.fields();
        read.sort_by(|a, b| a.name.cmp(&b.name));
        let mut want = fields;
        want.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(read, want);
        assert!(ilst.replace(&[field("TRACKNUMBER", &["three"])]).is_err());
    }
}
