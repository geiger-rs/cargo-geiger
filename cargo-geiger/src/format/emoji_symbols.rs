use crate::format::{Charset, SymbolKind};

use colored::Colorize;

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
