use crate::format::parse::Parser;
use crate::format::{Chunk, RawChunk};
use crate::mapping::CargoMetadataParameters;

use super::display::Display;

use cargo_metadata::PackageId;
use std::error::Error;

#[derive(Debug, PartialEq)]
pub struct Pattern {
    pub chunks: Vec<Chunk>,
}

impl Pattern {
    pub fn new(chunks: Vec<Chunk>) -> Self {
        Pattern { chunks }
    }

    pub fn display<'a>(
        &'a self,
        cargo_metadata_parameters: &'a CargoMetadataParameters,
        package: &'a PackageId,
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

        Ok(Pattern { chunks })
    }
}

#[cfg(test)]
mod pattern_tests {
    use super::*;
    use rstest::*;

    #[rstest(
        input_format_string,
        expected_pattern,
        case("{p}", Pattern::new(vec![Chunk::Package])),
        case("{l}", Pattern::new(vec![Chunk::License])),
        case("{r}", Pattern::new(vec![Chunk::Repository])),
        case("Text", Pattern::new(vec![Chunk::Raw(String::from("Text"))])),
        case(
            "{p}-{l}-{r}-Text",
            Pattern {
                chunks: vec! [
                    Chunk::Package,
                    Chunk::Raw(String::from("-")),
                    Chunk::License,
                    Chunk::Raw(String::from("-")),
                    Chunk::Repository,
                    Chunk::Raw(String::from("-Text"))
                ]
            }
        )
    )]
    fn pattern_try_build_test(
        input_format_string: &str,
        expected_pattern: Pattern,
    ) {
        let pattern_result = Pattern::try_build(input_format_string);
        assert!(pattern_result.is_ok());
        assert_eq!(pattern_result.unwrap(), expected_pattern);
    }
}
