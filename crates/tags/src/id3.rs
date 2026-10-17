//! ID3v2 frames as foobar2000 names them. Standard text frames get their
//! usual names, TXXX frames go by their description, and anything else that
//! isn't text (pictures, PRIV, chapters, …) is left alone.

use std::borrow::Cow;

use anyhow::Context;
use hifi_core::TagField;
use lofty::TextEncoding;
use lofty::id3::v2::{
    CommentFrame, ExtendedTextFrame, Frame, FrameId, Id3v2Tag, TextInformationFrame, TimestampFrame,
    UnsynchronizedTextFrame,
};
use lofty::tag::items::Timestamp;

use crate::{Native, push};

/// Text frames and their field names.
const TEXT: [(&str, &str); 31] = [
    ("TIT2", "TITLE"),
    ("TPE1", "ARTIST"),
    ("TALB", "ALBUM"),
    ("TPE2", "ALBUM ARTIST"),
    ("TCON", "GENRE"),
    ("TCOM", "COMPOSER"),
    ("TPE3", "CONDUCTOR"),
    ("TPE4", "REMIXER"),
    ("TEXT", "LYRICIST"),
    ("TIT1", "CONTENT GROUP"),
    ("TIT3", "SUBTITLE"),
    ("TPUB", "PUBLISHER"),
    ("TCOP", "COPYRIGHT"),
    ("TSRC", "ISRC"),
    ("TENC", "ENCODED BY"),
    ("TSSE", "ENCODING SETTINGS"),
    ("TBPM", "BPM"),
    ("TKEY", "INITIAL KEY"),
    ("TLAN", "LANGUAGE"),
    ("TMED", "MEDIA TYPE"),
    ("TMOO", "MOOD"),
    ("TOPE", "ORIGINAL ARTIST"),
    ("TOAL", "ORIGINAL ALBUM"),
    ("TOLY", "ORIGINAL LYRICIST"),
    ("TSOA", "ALBUMSORTORDER"),
    ("TSOP", "ARTISTSORTORDER"),
    ("TSOT", "TITLESORTORDER"),
    ("TSO2", "ALBUMARTISTSORTORDER"),
    ("TSOC", "COMPOSERSORTORDER"),
    ("TCMP", "ITUNESCOMPILATION"),
    ("TLEN", "LENGTH"),
];

/// Timestamp frames and their field names.
const DATES: [(&str, &str); 3] = [("TDRC", "DATE"), ("TDOR", "ORIGINAL RELEASE DATE"), ("TDRL", "RELEASE DATE")];

/// Number pairs ("3/12") and the two fields each holds.
const PAIRS: [(&str, &str, &str); 2] = [("TRCK", "TRACKNUMBER", "TOTALTRACKS"), ("TPOS", "DISCNUMBER", "TOTALDISCS")];

const COMMENT: &str = "COMMENT";
const LYRICS: &str = "UNSYNCED LYRICS";

fn name_of(id: &str) -> Option<&'static str> {
    TEXT.iter().chain(DATES.iter()).find(|(frame, _)| *frame == id).map(|(_, name)| *name)
}

fn frame_of(table: &[(&'static str, &'static str)], name: &str) -> Option<&'static str> {
    table.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)).map(|(frame, _)| *frame)
}

/// A field hifi shows for this frame; everything else is kept as is.
fn is_field(frame: &Frame<'_>) -> bool {
    match frame {
        Frame::Text(_) => true,
        Frame::Timestamp(f) => DATES.iter().any(|(id, _)| f.id().as_str() == *id),
        Frame::UserText(f) => !f.description.is_empty(),
        Frame::Comment(f) => f.description.is_empty(),
        Frame::UnsynchronizedText(f) => f.description.is_empty(),
        _ => false,
    }
}

impl Native for Id3v2Tag {
    fn fields(&self) -> Vec<TagField> {
        let mut fields = Vec::new();
        for frame in self {
            if !is_field(frame) {
                continue;
            }
            match frame {
                Frame::Text(f) => {
                    let id = f.id().as_str();
                    if let Some((_, number, total)) = PAIRS.iter().find(|(pair, ..)| *pair == id) {
                        let (n, t) = f.value.split_once('/').unwrap_or((&f.value, ""));
                        push(&mut fields, number, [n.trim()]);
                        push(&mut fields, total, [t.trim()]);
                    } else {
                        push(&mut fields, name_of(id).unwrap_or(id), f.value.split('\0'));
                    }
                }
                Frame::Timestamp(f) => {
                    let name = name_of(f.id().as_str()).expect("is_field checked it");
                    push(&mut fields, name, [f.timestamp.to_string()]);
                }
                Frame::UserText(f) => push(&mut fields, &f.description, f.content.split('\0')),
                Frame::Comment(f) => push(&mut fields, COMMENT, [f.content.as_ref()]),
                Frame::UnsynchronizedText(f) => push(&mut fields, LYRICS, [f.content.as_ref()]),
                _ => {}
            }
        }
        fields
    }

    fn replace(&mut self, fields: &[TagField]) -> anyhow::Result<()> {
        self.retain(|frame| !is_field(frame));
        let first = |name: &str| {
            fields.iter().find(|f| f.name.eq_ignore_ascii_case(name)).and_then(|f| f.values.first()).map(String::as_str)
        };
        for (id, number, total) in PAIRS {
            let value = match (first(number), first(total)) {
                (Some(n), Some(t)) => format!("{n}/{t}"),
                (Some(n), None) => n.to_string(),
                // A total without a number has no place in TRCK/TPOS.
                (None, Some(t)) => {
                    self.insert(user_text(total, t));
                    continue;
                }
                (None, None) => continue,
            };
            self.insert(text(id, value)?);
        }
        for field in fields {
            let name = field.name.as_str();
            if PAIRS.iter().any(|(_, n, t)| n.eq_ignore_ascii_case(name) || t.eq_ignore_ascii_case(name)) {
                continue;
            }
            let joined = field.values.join("\0");
            let frame = if name.eq_ignore_ascii_case(COMMENT) {
                Frame::Comment(CommentFrame::new(TextEncoding::UTF8, *b"eng", "", field.values.join("\n")))
            } else if name.eq_ignore_ascii_case(LYRICS) {
                Frame::UnsynchronizedText(UnsynchronizedTextFrame::new(
                    TextEncoding::UTF8,
                    *b"eng",
                    "",
                    field.values.join("\n"),
                ))
            } else if let Some(id) = frame_of(&DATES, name) {
                let value = &field.values[0];
                let timestamp: Timestamp = value.parse().ok().with_context(|| {
                    format!("{name} must be a date like 2015 or 2015-04-01 in MP3 files, not \"{value}\"")
                })?;
                Frame::Timestamp(TimestampFrame::new(FrameId::new(id)?, TextEncoding::UTF8, timestamp))
            } else if let Some(id) = frame_of(&TEXT, name) {
                text(id, joined)?
            } else if is_text_frame_id(name) {
                // A text frame hifi has no name for, shown under its ID.
                text(name, joined)?
            } else {
                user_text(name, &joined)
            };
            self.insert(frame);
        }
        Ok(())
    }
}

fn text(id: &str, value: String) -> anyhow::Result<Frame<'static>> {
    let id = FrameId::new(Cow::Owned(id.to_string()))?;
    Ok(Frame::Text(TextInformationFrame::new(id, TextEncoding::UTF8, value)))
}

fn user_text(description: &str, value: &str) -> Frame<'static> {
    Frame::UserText(ExtendedTextFrame::new(TextEncoding::UTF8, description.to_string(), value.to_string()))
}

fn is_text_frame_id(name: &str) -> bool {
    name.len() == 4
        && name.starts_with('T')
        && name != "TXXX"
        && name.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str, values: &[&str]) -> TagField {
        TagField { name: name.into(), values: values.iter().map(|v| v.to_string()).collect() }
    }

    #[test]
    fn maps_frames_to_foobar_names_and_back() {
        let fields = vec![
            field("TITLE", &["Conta"]),
            field("ARTIST", &["A", "B"]),
            field("ALBUM ARTIST", &["Various"]),
            field("DATE", &["2015-04-01"]),
            field("TRACKNUMBER", &["3"]),
            field("TOTALTRACKS", &["12"]),
            field("DISCNUMBER", &["1"]),
            field("COMMENT", &["nice"]),
            field("MusicBrainz Album Id", &["abc"]),
            field("TKEY", &["Am"]),
        ];
        let mut tag = Id3v2Tag::default();
        tag.replace(&fields).unwrap();
        assert_eq!(tag.get_text(&FrameId::new("TRCK").unwrap()), Some("3/12"));
        assert_eq!(tag.get_text(&FrameId::new("TPOS").unwrap()), Some("1"));
        assert_eq!(tag.get_user_text("MusicBrainz Album Id"), Some("abc"));

        let mut read = tag.fields();
        read.sort_by(|a, b| a.name.cmp(&b.name));
        let mut want = fields;
        // TKEY has a name.
        want.iter_mut().find(|f| f.name == "TKEY").unwrap().name = "INITIAL KEY".into();
        want.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(read, want);
    }

    #[test]
    fn keeps_frames_it_does_not_show() {
        let mut tag = Id3v2Tag::default();
        tag.insert(Frame::Comment(CommentFrame::new(TextEncoding::UTF8, *b"eng", "iTunNORM", "0000")));
        tag.replace(&[field("TITLE", &["x"])]).unwrap();
        assert_eq!((&tag).into_iter().filter(|f| matches!(f, Frame::Comment(_))).count(), 1);
        assert_eq!(tag.fields(), vec![field("TITLE", &["x"])]);
    }

    #[test]
    fn refuses_dates_id3_cannot_hold() {
        let mut tag = Id3v2Tag::default();
        assert!(tag.replace(&[field("DATE", &["sometime in 2015"])]).is_err());
    }
}
