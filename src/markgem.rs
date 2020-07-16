use anyhow::Result;
use pulldown_cmark::{CowStr, Event, Parser, Tag};
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

struct Link<'a> {
    destination: CowStr<'a>,
    // Note that this is *not* the text that appears in square brackets in markdown, but a separate
    // title attribute.
    title: CowStr<'a>,
}

struct Converter<'a, W: Write> {
    out: BufWriter<W>,
    // We need to keep track of this so we can write `[1]` footnote markers or similar with text.
    next_link_id: usize,
    links: Vec<Link<'a>>,
}

impl<'a, W: Write> Converter<'a, W> {
    fn new(writer: W) -> Self {
        Self {
            out: BufWriter::new(writer),
            next_link_id: 1,
            links: vec![],
        }
    }
    fn convert(mut self, parser: Parser<'a>) -> Result<()> {
        for event in parser {
            match event {
                Event::Start(Tag::Emphasis) | Event::End(Tag::Emphasis) => self.write("*")?,
                Event::Start(Tag::Strong) | Event::End(Tag::Strong) => self.write("**")?,
                Event::Start(Tag::BlockQuote) => self.write(">")?,
                // TODO: Nested lists, properly dealing with ordered lists.
                Event::Start(Tag::Item) => self.write("* ")?,
                Event::End(Tag::Item) => self.write("\n")?,
                Event::End(Tag::List(_)) => self.write("\n")?,
                Event::Start(Tag::Heading(depth)) => {
                    self.out
                        // Max out at 3, since that's the most Gemtext supports.
                        .write_all(vec![b'#'; depth.max(3) as usize].as_slice())?;
                    self.write(" ")?
                }
                Event::End(Tag::Heading(_)) => self.write("\n\n")?,
                Event::End(Tag::Paragraph) => {
                    self.write("\n\n")?;
                    self.write_pending_links()?
                }
                Event::End(Tag::Link(_, destination, title)) => {
                    self.handle_link(destination, title)?
                }
                Event::Text(text) => {
                    self.write(&text)?;
                    self.write(" ")?
                }
                _ => (),
            }
        }
        Ok(())
    }

    fn handle_link(&mut self, destination: CowStr<'a>, title: CowStr<'a>) -> Result<()> {
        self.links.push(Link { destination, title });
        self.write(&format!("[{}]", self.next_link_id))?;
        self.next_link_id += 1;
        Ok(())
    }

    /// Writes all of the links in `self.links`. Adds additional padding if any links were written.
    fn write_pending_links(&mut self) -> Result<()> {
        if self.links.is_empty() {
            return Ok(());
        }
        let links = std::mem::replace(&mut self.links, vec![]);
        for link in links {
            self.write("=> ")?;
            self.write(&link.destination)?;
            self.write(" ")?;
            self.write(&link.title)?;
            self.write("\n")?;
        }
        self.write("\n")?;
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
