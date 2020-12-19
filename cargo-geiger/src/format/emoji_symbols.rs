use crate::format::print_config::{colorize, OutputFormat};
use crate::format::{CrateDetectionStatus, SymbolKind};

use colored::ColoredString;

pub struct EmojiSymbols {
    emojis: [&'static str; 3],
    fallbacks: [ColoredString; 3],
    output_format: OutputFormat,
}

impl EmojiSymbols {
    pub fn emoji(&self, kind: SymbolKind) -> Box<dyn std::fmt::Display> {
        let idx = kind as usize;
        if self.will_output_emoji() {
            Box::new(self.emojis[idx])
        } else {
            Box::new(self.fallbacks[idx].clone())
        }
    }

    pub fn new(output_format: OutputFormat) -> EmojiSymbols {
        Self {
            emojis: ["🔒", "❓", "☢️"],
            fallbacks: [
                colorize(
                    &CrateDetectionStatus::NoneDetectedForbidsUnsafe,
                    output_format,
                    String::from(":)"),
                ),
                colorize(
                    &CrateDetectionStatus::NoneDetectedAllowsUnsafe,
                    output_format,
                    String::from("?"),
                ),
                colorize(
                    &CrateDetectionStatus::UnsafeDetected,
                    output_format,
                    String::from("!"),
                ),
            ],
            output_format,
        }
    }

    pub fn will_output_emoji(&self) -> bool {
        (self.output_format == OutputFormat::Utf8
            && console::Term::stdout().features().wants_emoji())
            || self.output_format == OutputFormat::GitHubMarkdown
    }
}
