mod parse;
pub mod print;
pub mod table;
pub mod tree;

use cargo::core::dependency::DepKind;
use cargo::core::manifest::ManifestMetadata;
use cargo::core::PackageId;
use colored::Colorize;
use std::error::Error;
use std::fmt;
use std::str::{self, FromStr};

use self::parse::{Parser, RawChunk};

// ---------- BEGIN: Public items ----------

#[derive(Clone, Copy, PartialEq)]
pub enum Charset {
    Utf8,
    Ascii,
}

impl FromStr for Charset {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Charset, &'static str> {
        match s {
            "utf8" => Ok(Charset::Utf8),
            "ascii" => Ok(Charset::Ascii),
            _ => Err("invalid charset"),
        }
    }
}

pub struct Display<'a> {
    pattern: &'a Pattern,
    package: &'a PackageId,
    metadata: &'a ManifestMetadata,
}

impl<'a> fmt::Display for Display<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        for chunk in &self.pattern.0 {
            match *chunk {
                Chunk::Raw(ref s) => (fmt.write_str(s))?,
                Chunk::Package => {
                    (write!(
                        fmt,
                        "{} {}",
                        self.package.name(),
                        self.package.version()
                    ))?
                }
                Chunk::License => {
                    if let Some(ref license) = self.metadata.license {
                        (write!(fmt, "{}", license))?
                    }
                }
                Chunk::Repository => {
                    if let Some(ref repository) = self.metadata.repository {
                        (write!(fmt, "{}", repository))?
                    }
                }
            }
        }

        Ok(())
    }
}

pub struct EmojiSymbols {
    charset: Charset,
    emojis: [&'static str; 3],
    fallbacks: [colored::ColoredString; 3],
}

impl EmojiSymbols {
    pub fn new(charset: Charset) -> EmojiSymbols {
        Self {
            charset,
            emojis: ["🔒", "❓", "☢️"],
            fallbacks: [":)".green(), "?".normal(), "!".red().bold()],
        }
    }

    pub fn will_output_emoji(&self) -> bool {
        self.charset == Charset::Utf8
            && console::Term::stdout().features().wants_emoji()
    }

    pub fn emoji(&self, kind: SymbolKind) -> Box<dyn std::fmt::Display> {
        let idx = kind as usize;
        if self.will_output_emoji() {
            Box::new(self.emojis[idx])
        } else {
            Box::new(self.fallbacks[idx].clone())
        }
    }
}

pub struct Pattern(Vec<Chunk>);

impl Pattern {
    pub fn try_build(format: &str) -> Result<Pattern, Box<dyn Error>> {
        let mut chunks = vec![];

        for raw in Parser::new(format) {
            let chunk = match raw {
                RawChunk::Text(text) => Chunk::Raw(text.to_owned()),
                RawChunk::Argument("p") => Chunk::Package,
                RawChunk::Argument("l") => Chunk::License,
                RawChunk::Argument("r") => Chunk::Repository,
                RawChunk::Argument(ref a) => {
                    return Err(format!("unsupported pattern `{}`", a).into());
                }
                RawChunk::Error(err) => return Err(err.into()),
            };
            chunks.push(chunk);
        }

        Ok(Pattern(chunks))
    }

    pub fn display<'a>(
        &'a self,
        package: &'a PackageId,
        metadata: &'a ManifestMetadata,
    ) -> Display<'a> {
        Display {
            pattern: self,
            package,
            metadata,
        }
    }
}

#[derive(Clone, Copy)]
pub enum SymbolKind {
    Lock = 0,
    QuestionMark = 1,
    Rads = 2,
}

pub fn get_kind_group_name(k: DepKind) -> Option<&'static str> {
    match k {
        DepKind::Normal => None,
        DepKind::Build => Some("[build-dependencies]"),
        DepKind::Development => Some("[dev-dependencies]"),
    }
}

// ---------- END: Public items ----------

enum Chunk {
    Raw(String),
    Package,
    License,
    Repository,
}
