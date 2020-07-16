use anyhow::Result;
use pulldown_cmark::{Event, Parser, Tag};
use std::io::{BufWriter, Write};

/// Converts the given Markdown to Gemini, writing it to the given output. The output will be
/// automatically buffered.
pub fn to_gemini(markdown: &str) -> Result<Vec<u8>> {
    let markdown = strip_matter(markdown);
    let mut vec: Vec<u8> = vec![];
    let converter = Converter::new(&mut vec);
    converter.convert(Parser::new(markdown))?;
    Ok(vec)
}

struct Converter<W: Write> {
    out: BufWriter<W>,
}

impl<W: Write> Converter<W> {
    fn new(writer: W) -> Self {
        Self {
            out: BufWriter::new(writer),
        }
    }
    fn convert(mut self, parser: Parser) -> Result<()> {
        for event in parser {
            match event {
                Event::Start(Tag::Emphasis) | Event::End(Tag::Emphasis) => self.write("*")?,
                Event::Start(Tag::Strong) | Event::End(Tag::Strong) => self.write("**")?,
                Event::Start(Tag::Heading(depth)) => {
                    self.out.write_all(vec![b'#'; depth as usize].as_slice())?;
                    self.write(" ")?
                }
                Event::End(Tag::Heading(_)) => self.write("\n\n")?,
                Event::End(Tag::Paragraph) => self.write("\n\n")?,
                Event::Text(text) => self.write(&text)?,
                _ => (),
            }
        }
        Ok(())
    }

    fn write(&mut self, s: &str) -> Result<()> {
        self.out.write_all(s.as_bytes())?;
        Ok(())
    }
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
