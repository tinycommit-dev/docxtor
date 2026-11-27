//! The spelling checker the editor's spelling worker uses (`packages/editor/src/spelling/worker.ts`): one Hunspell dictionary,
//! read by [spellbook](https://github.com/helix-editor/spellbook), which checks a word by taking its affixes off as Hunspell does
//! instead of listing every form of every word up front.

use wasm_bindgen::prelude::*;

/// One dictionary.
#[wasm_bindgen]
pub struct Speller {
    dict: spellbook::Dictionary,
}

#[wasm_bindgen]
impl Speller {
    /// Read a dictionary from its affix file and its word list, both as text.
    #[wasm_bindgen(constructor)]
    pub fn new(aff: &str, dic: &str) -> Result<Speller, JsError> {
        spellbook::Dictionary::new(aff, dic).map(|dict| Speller { dict }).map_err(|e| JsError::new(&format!("The dictionary can't be read: {e}")))
    }

    /// Whether a word is spelt right.
    pub fn check(&self, word: &str) -> bool {
        self.dict.check(word)
    }

    /// Check words given one a line: one byte a word, 1 when it is spelt right.
    #[wasm_bindgen(js_name = checkLines)]
    pub fn check_lines(&self, words: &str) -> Vec<u8> {
        words.split('\n').map(|w| u8::from(self.dict.check(w))).collect()
    }

    /// Corrections for a word, best first.
    pub fn suggest(&self, word: &str) -> Vec<String> {
        let mut out = Vec::new();
        self.dict.suggest(word, &mut out);
        out
    }

    /// Add a word, written as a dictionary line (`chatbot/MS`, the flags giving its endings) or alone. False when its flags can't
    /// be read.
    pub fn add(&mut self, entry: &str) -> bool {
        self.dict.add(entry).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AFF: &str = "SET UTF-8\nSFX S Y 1\nSFX S 0 s .\n";
    const DIC: &str = "2\nrobot/S\nhouse/S\n";

    #[test]
    fn a_dictionary_of_a_few_words_with_no_rules() {
        let s = Speller::new("SET UTF-8\n", "2\nciao\nmondo\n").ok().unwrap();
        assert_eq!(s.check_lines("ciao\nmondo\nfoo"), vec![1, 1, 0]);
    }

    #[test]
    fn checks_suggests_and_adds() {
        let mut s = Speller::new(AFF, DIC).ok().unwrap();
        assert!(s.check("robots") && s.check("house") && !s.check("hous"));
        assert_eq!(s.check_lines("robot\nrobotz\nhouses"), vec![1, 0, 1]);
        assert!(s.suggest("robit").iter().any(|w| w == "robot"));
        assert!(!s.check("chatbots"));
        assert!(s.add("chatbot/S"));
        assert!(s.check("chatbots"));
    }
}
