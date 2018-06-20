use cargo::core::PackageId;
use cargo::core::manifest::ManifestMetadata;
use std::error::Error;
use std::fmt;

use format::parse::{Parser, RawChunk};

mod parse;

enum Chunk {
    Raw(String),
    Package,
    License,
    Repository,
}

pub struct Pattern(Vec<Chunk>);

impl Pattern {
    pub fn new(format: &str) -> Result<Pattern, Box<Error>> {
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
            package: package,
            metadata: metadata,
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
                Chunk::Raw(ref s) => try!(fmt.write_str(s)),
                Chunk::Package => try!(write!(fmt, "{}", self.package)),
                Chunk::License => if let Some(ref license) = self.metadata.license {
                    try!(write!(fmt, "{}", license))
                },
                Chunk::Repository => if let Some(ref repository) = self.metadata.repository {
                    try!(write!(fmt, "{}", repository))
                },
            }
        }

        Ok(())
    }
}
