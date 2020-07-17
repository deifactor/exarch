use anyhow::Result;
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag};
use std::io::{BufWriter, Write};

/// Converts the given Markdown to Gemini, writing it to the given output. The output will be
/// automatically buffered.
pub fn to_gemini(markdown: &str) -> Result<Vec<u8>> {
    let markdown = strip_matter(markdown);
    let mut vec: Vec<u8> = vec![];
    let converter = Converter::new(&mut vec);
    converter.convert(Parser::new_ext(markdown, Options::ENABLE_STRIKETHROUGH))?;
    while vec.last() == Some(&b'\n') {
        vec.pop();
    }
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
                Event::Start(Tag::Strikethrough) | Event::End(Tag::Strikethrough) => {
                    self.write("~~")?
                }
                Event::Start(Tag::BlockQuote) => self.write(">")?,
                // TODO: Nested lists, properly dealing with ordered lists.
                Event::Start(Tag::Item) => self.write("* ")?,
                Event::End(Tag::Item) => self.write("\n")?,
                Event::End(Tag::List(_)) => self.write("\n")?,
                Event::Start(Tag::Heading(depth)) => {
                    self.out
                        // Max out at 3, since that's the most Gemtext supports.
                        .write_all(vec![b'#'; depth.min(3) as usize].as_slice())?;
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
                }
                Event::SoftBreak => self.write(" ")?,
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
            if !link.title.is_empty() {
                self.write(" ")?;
                self.write(&link.title)?;
            }
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

#[cfg(test)]
mod test {
    use super::*;
    use indoc::indoc;

    fn check_conversion(markdown: &str, gemini: &str) -> Result<()> {
        let bytes = to_gemini(markdown)?;
        assert_eq!(gemini, String::from_utf8(bytes)?);
        Ok(())
    }

    #[test]
    fn soft_newline() -> Result<()> {
        check_conversion("foo\nbar", "foo bar")
    }

    #[test]
    fn hard_newline() -> Result<()> {
        check_conversion("foo\n\nbar", "foo\n\nbar")
    }

    #[test]
    fn headers() -> Result<()> {
        check_conversion(
            "# one\n## two\n### three\n#### four",
            // caps out at depth three
            "# one\n\n## two\n\n### three\n\n### four",
        )
    }

    mod lists {
        use super::*;

        #[test]
        fn unordered() -> Result<()> {
            for marker in &["*", "-", "+"] {
                check_conversion(
                    &format!("{marker} first\n{marker} second", marker = marker),
                    "* first\n* second",
                )?;
            }
            Ok(())
        }

        #[test]
        fn ordered_list() -> Result<()> {
            check_conversion("1. foo\n1. bar\n1. baz", "1. foo\n2. bar\n3. baz")
        }
    }

    mod links {
        use super::*;
        #[test]
        fn one_paragraph() -> Result<()> {
            let markdown =
                "here is [a link](http://first.com). here is [another](http://second.com).";
            let gemini = indoc!(
                r#"
            here is a link[1]. here is another[2].

            => http://first.com
            => http://second.com"#
            );
            check_conversion(markdown, gemini)
        }

        #[test]
        fn multiple_paragraphs() -> Result<()> {
            let markdown = indoc!(
                "
            here is [a link](http://first.com).

            here's some intervening text.

            and [another link](http://second.com).

            more text!

            this is [the last link](http://third.com).
            "
            );
            let gemini = indoc!(
                "
            here is a link[1].

            => http://first.com

            here's some intervening text.

            and another link[2].

            => http://second.com

            more text!

            this is the last link[3].

            => http://third.com"
            );
            check_conversion(markdown, gemini)
        }
    }
}
