use anyhow::Result;
use pulldown_cmark::{Event, Options, Parser, Tag};
use std::io::{BufWriter, Write};

/// Converts the given Markdown to Gemini, writing it to the given output. The output will be
/// automatically buffered.
pub fn to_gemini(markdown: &str) -> Result<Vec<u8>> {
    let markdown = strip_matter(markdown);
    let parser = Parser::new(markdown);
    let mut vec: Vec<u8> = vec![];
    {
        let mut out = BufWriter::new(&mut vec);
        for event in parser {
            match event {
                Event::Text(text) => out.write_all(text.as_bytes())?,
                Event::End(Tag::Paragraph) => out.write_all(b"\n\n")?,
                _ => (),
            }
        }
    }
    Ok(vec)
}

/// Removes the Zola front matter from some markdown text. The front matter is delimited by +++
/// symbols.
fn strip_matter(markdown: &str) -> &str {
    let splits: Vec<_> = markdown.splitn(3, "+++").collect();
    match splits.len() {
        1 => splits[0],
        2 => splits[1],
        3 => splits[2],
        _ => markdown,
    }
}
