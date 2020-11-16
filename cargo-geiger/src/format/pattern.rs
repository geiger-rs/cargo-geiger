use crate::format::parse::Parser;
use crate::format::{Chunk, RawChunk};

use super::display::Display;

use crate::mapping::CargoMetadataParameters;
use std::error::Error;

#[derive(Debug, PartialEq)]
pub struct Pattern(pub Vec<Chunk>);

impl Pattern {
    pub fn display<'a>(
        &'a self,
        cargo_metadata_parameters: &'a CargoMetadataParameters,
        package: &'a cargo_metadata::PackageId,
    ) -> Display<'a> {
        Display {
            cargo_metadata_parameters,
            pattern: self,
            package,
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
