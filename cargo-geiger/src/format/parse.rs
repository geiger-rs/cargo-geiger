use crate::format::RawChunk;

use std::{iter, str};

pub struct Parser<'a> {
    s: &'a str,
    it: iter::Peekable<str::CharIndices<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(s: &'a str) -> Parser<'a> {
        Parser {
            s,
            it: s.char_indices().peekable(),
        }
    }

    fn argument(&mut self) -> RawChunk<'a> {
        RawChunk::Argument(self.name())
    }

    fn consume(&mut self, ch: char) -> bool {
        match self.it.peek() {
            Some(&(_, c)) if c == ch => {
                self.it.next();
                true
            }
            _ => false,
        }
    }

    fn name(&mut self) -> &'a str {
        let start = match self.it.peek() {
            Some(&(pos, ch)) if ch.is_alphabetic() => {
                self.it.next();
                pos
            }
            _ => return "",
        };

        loop {
            match self.it.peek() {
                Some(&(_, ch)) if ch.is_alphanumeric() => {
                    self.it.next();
                }
                Some(&(end, _)) => return &self.s[start..end],
                None => return &self.s[start..],
            }
        }
    }

    fn text(&mut self, start: usize) -> RawChunk<'a> {
        while let Some(&(pos, ch)) = self.it.peek() {
            match ch {
                '{' | '}' | ')' => return RawChunk::Text(&self.s[start..pos]),
                _ => {
                    self.it.next();
                }
            }
        }
        RawChunk::Text(&self.s[start..])
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = RawChunk<'a>;

    fn next(&mut self) -> Option<RawChunk<'a>> {
        match self.it.peek() {
            Some(&(_, '{')) => {
                self.it.next();
                if self.consume('{') {
                    Some(RawChunk::Text("{"))
                } else {
                    let chunk = self.argument();
                    if self.consume('}') {
                        Some(chunk)
                    } else {
                        for _ in &mut self.it {}
                        Some(RawChunk::Error("expected '}'"))
                    }
                }
            }
            Some(&(_, '}')) => {
                self.it.next();
                Some(RawChunk::Error("unexpected '}'"))
            }
            Some(&(i, _)) => Some(self.text(i)),
            None => None,
        }
    }
}

#[cfg(test)]
pub mod parse_tests {
    use super::*;

    use rstest::*;

    #[rstest]
    fn parser_new_test() {
        let parser_s = "parser_string";
        let parser = Parser::new(parser_s);
        assert_eq!(parser.s, parser_s);
    }

    #[rstest]
    fn parser_argument_test() {
        let mut parser = Parser::new("parser 1.2.3");
        let raw_chunk = parser.argument();
        assert_eq!(raw_chunk, RawChunk::Argument("parser"));
    }

    #[rstest(
        input_s_string,
        expected_name_string,
        case("parser 1.2.3", "parser"),
        case("1.2.3 parser", "")
    )]
    fn parser_name_test(input_s_string: &str, expected_name_string: &str) {
        let mut parser = Parser::new(input_s_string);
        assert_eq!(parser.name(), expected_name_string)
    }

    #[rstest]
    fn parser_text_test() {
        let parser_s = "parser 1.2.3";
        let mut parser = Parser::new(parser_s);
        assert_eq!(parser.text(0), RawChunk::Text(parser_s));
    }
}
