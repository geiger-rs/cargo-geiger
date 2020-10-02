use crate::format::parse::Parser;
use crate::format::{Chunk, RawChunk};

use super::display::Display;

use cargo::core::manifest::ManifestMetadata;
use cargo::core::PackageId;
use std::error::Error;

pub struct Pattern(pub Vec<Chunk>);

impl Pattern {
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
}
