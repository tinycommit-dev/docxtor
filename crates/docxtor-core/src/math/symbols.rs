//! Tables shared by the LaTeX reader, the LaTeX writer and the MathML writer.

/// `\name` to the character it stands for. The first entry for a character is the one the writer uses.
pub const SYMBOLS: &[(&str, &str)] = &[
    // Greek, lower case
    ("alpha", "α"),
    ("beta", "β"),
    ("gamma", "γ"),
    ("delta", "δ"),
    ("epsilon", "ϵ"),
    ("varepsilon", "ε"),
    ("zeta", "ζ"),
    ("eta", "η"),
    ("theta", "θ"),
    ("vartheta", "ϑ"),
    ("iota", "ι"),
    ("kappa", "κ"),
    ("lambda", "λ"),
    ("mu", "μ"),
    ("nu", "ν"),
    ("xi", "ξ"),
    ("pi", "π"),
    ("varpi", "ϖ"),
    ("rho", "ρ"),
    ("varrho", "ϱ"),
    ("sigma", "σ"),
    ("varsigma", "ς"),
    ("tau", "τ"),
    ("upsilon", "υ"),
    ("phi", "ϕ"),
    ("varphi", "φ"),
    ("chi", "χ"),
    ("psi", "ψ"),
    ("omega", "ω"),
    // Greek, upper case (the others are Latin letters)
    ("Gamma", "Γ"),
    ("Delta", "Δ"),
    ("Theta", "Θ"),
    ("Lambda", "Λ"),
    ("Xi", "Ξ"),
    ("Pi", "Π"),
    ("Sigma", "Σ"),
    ("Upsilon", "Υ"),
    ("Phi", "Φ"),
    ("Psi", "Ψ"),
    ("Omega", "Ω"),
    // Operators and relations
    ("cdot", "⋅"),
    ("times", "×"),
    ("div", "÷"),
    ("pm", "±"),
    ("mp", "∓"),
    ("leq", "≤"),
    ("le", "≤"),
    ("geq", "≥"),
    ("ge", "≥"),
    ("neq", "≠"),
    ("ne", "≠"),
    ("approx", "≈"),
    ("equiv", "≡"),
    ("sim", "∼"),
    ("simeq", "≃"),
    ("cong", "≅"),
    ("propto", "∝"),
    ("ll", "≪"),
    ("gg", "≫"),
    ("circ", "∘"),
    ("bullet", "∙"),
    ("ast", "∗"),
    ("star", "⋆"),
    ("oplus", "⊕"),
    ("otimes", "⊗"),
    ("dagger", "†"),
    ("mid", "∣"),
    ("parallel", "∥"),
    ("perp", "⊥"),
    ("angle", "∠"),
    ("degree", "°"),
    // Arrows
    ("to", "→"),
    ("rightarrow", "→"),
    ("leftarrow", "←"),
    ("gets", "←"),
    ("leftrightarrow", "↔"),
    ("Rightarrow", "⇒"),
    ("implies", "⇒"),
    ("Leftarrow", "⇐"),
    ("Leftrightarrow", "⇔"),
    ("iff", "⇔"),
    ("mapsto", "↦"),
    ("uparrow", "↑"),
    ("downarrow", "↓"),
    // Sets and logic
    ("in", "∈"),
    ("notin", "∉"),
    ("ni", "∋"),
    ("subset", "⊂"),
    ("supset", "⊃"),
    ("subseteq", "⊆"),
    ("supseteq", "⊇"),
    ("nsubseteq", "⊈"),
    ("cup", "∪"),
    ("cap", "∩"),
    ("setminus", "∖"),
    ("emptyset", "∅"),
    ("varnothing", "∅"),
    ("forall", "∀"),
    ("exists", "∃"),
    ("nexists", "∄"),
    ("neg", "¬"),
    ("lnot", "¬"),
    ("wedge", "∧"),
    ("land", "∧"),
    ("vee", "∨"),
    ("lor", "∨"),
    ("therefore", "∴"),
    ("because", "∵"),
    // Other
    ("infty", "∞"),
    ("partial", "∂"),
    ("nabla", "∇"),
    ("ldots", "…"),
    ("dots", "…"),
    ("cdots", "⋯"),
    ("vdots", "⋮"),
    ("ddots", "⋱"),
    ("prime", "′"),
    ("hbar", "ℏ"),
    ("ell", "ℓ"),
    ("Re", "ℜ"),
    ("Im", "ℑ"),
    ("aleph", "ℵ"),
    ("square", "□"),
    ("triangle", "△"),
    // Brackets used as plain characters
    ("langle", "⟨"),
    ("rangle", "⟩"),
    ("lfloor", "⌊"),
    ("rfloor", "⌋"),
    ("lceil", "⌈"),
    ("rceil", "⌉"),
    ("lbrace", "{"),
    ("rbrace", "}"),
    ("vert", "|"),
    ("Vert", "‖"),
];

/// Large operators: `\name`, the character, and whether limits go above and below by default.
pub const NARY: &[(&str, &str, bool)] = &[
    ("sum", "∑", true),
    ("prod", "∏", true),
    ("coprod", "∐", true),
    ("bigcup", "⋃", true),
    ("bigcap", "⋂", true),
    ("bigoplus", "⨁", true),
    ("bigotimes", "⨂", true),
    ("bigvee", "⋁", true),
    ("bigwedge", "⋀", true),
    ("int", "∫", false),
    ("iint", "∬", false),
    ("iiint", "∭", false),
    ("oint", "∮", false),
    ("oiint", "∯", false),
];

/// Function names that are written upright: `\sin` and friends. The flag says the subscript goes under the name.
pub const FUNCTIONS: &[(&str, bool)] = &[
    ("sin", false),
    ("cos", false),
    ("tan", false),
    ("cot", false),
    ("sec", false),
    ("csc", false),
    ("arcsin", false),
    ("arccos", false),
    ("arctan", false),
    ("sinh", false),
    ("cosh", false),
    ("tanh", false),
    ("coth", false),
    ("log", false),
    ("ln", false),
    ("lg", false),
    ("exp", false),
    ("deg", false),
    ("dim", false),
    ("ker", false),
    ("arg", false),
    ("hom", false),
    ("lim", true),
    ("limsup", true),
    ("liminf", true),
    ("max", true),
    ("min", true),
    ("sup", true),
    ("inf", true),
    ("det", true),
    ("gcd", true),
    ("Pr", true),
];

/// Accent commands and the combining character Word uses for them.
pub const ACCENTS: &[(&str, &str)] = &[
    ("hat", "\u{302}"),
    ("widehat", "\u{302}"),
    ("check", "\u{30C}"),
    ("tilde", "\u{303}"),
    ("widetilde", "\u{303}"),
    ("acute", "\u{301}"),
    ("grave", "\u{300}"),
    ("dot", "\u{307}"),
    ("ddot", "\u{308}"),
    ("breve", "\u{306}"),
    ("bar", "\u{304}"),
    ("vec", "\u{20D7}"),
    ("mathring", "\u{30A}"),
];

/// A delimiter after `\left`, `\right` or `\middle`: what is written, and the character it stands for
/// (empty for `.`, no delimiter).
pub fn delimiter(token: &str) -> Option<&'static str> {
    Some(match token {
        "(" => "(",
        ")" => ")",
        "[" => "[",
        "]" => "]",
        "{" | "lbrace" => "{",
        "}" | "rbrace" => "}",
        "|" | "vert" | "lvert" | "rvert" => "|",
        "Vert" | "lVert" | "rVert" => "‖",
        "langle" => "⟨",
        "rangle" => "⟩",
        "lfloor" => "⌊",
        "rfloor" => "⌋",
        "lceil" => "⌈",
        "rceil" => "⌉",
        "/" => "/",
        "." => "",
        _ => return None,
    })
}

/// The `\command` (without the backslash) for a delimiter character, as the writer spells it after `\left`.
pub fn delimiter_name(ch: &str) -> String {
    match ch {
        "" => ".".into(),
        "{" => "\\{".into(),
        "}" => "\\}".into(),
        "‖" => "\\|".into(),
        "⟨" => "\\langle".into(),
        "⟩" => "\\rangle".into(),
        "⌊" => "\\lfloor".into(),
        "⌋" => "\\rfloor".into(),
        "⌈" => "\\lceil".into(),
        "⌉" => "\\rceil".into(),
        other => other.into(),
    }
}

pub fn symbol(name: &str) -> Option<&'static str> {
    SYMBOLS.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

/// The preferred command for a character.
pub fn symbol_name(ch: &str) -> Option<&'static str> {
    SYMBOLS.iter().find(|(_, c)| *c == ch).map(|(n, _)| *n)
}

pub fn nary(name: &str) -> Option<(&'static str, bool)> {
    NARY.iter().find(|(n, _, _)| *n == name).map(|(_, c, u)| (*c, *u))
}

pub fn nary_name(ch: &str) -> Option<&'static str> {
    NARY.iter().find(|(_, c, _)| *c == ch).map(|(n, _, _)| *n)
}

/// Whether limits of this large operator go above and below by default (sums) or beside it (integrals).
pub fn nary_default_under_over(ch: &str) -> bool {
    NARY.iter().find(|(_, c, _)| *c == ch).map(|(_, _, u)| *u).unwrap_or(false)
}

pub fn function(name: &str) -> Option<bool> {
    FUNCTIONS.iter().find(|(n, _)| *n == name).map(|(_, l)| *l)
}

pub fn accent(name: &str) -> Option<&'static str> {
    ACCENTS.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

pub fn accent_name(ch: &str) -> Option<&'static str> {
    ACCENTS.iter().find(|(_, c)| *c == ch).map(|(n, _)| *n)
}

/// A spacing command (`\,`, `\quad`) and the space character Word keeps for it.
pub const SPACES: &[(&str, &str)] = &[(",", "\u{2009}"), (";", "\u{2005}"), (":", "\u{2005}"), (">", "\u{2005}"), ("quad", "\u{2003}"), ("qquad", "\u{2003}\u{2003}"), (" ", "\u{00A0}")];

pub fn space(name: &str) -> Option<&'static str> {
    SPACES.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

/// Double-struck capitals and the usual script capitals for `\mathbb` and `\mathcal`.
pub fn blackboard(c: char) -> Option<char> {
    Some(match c {
        'C' => 'ℂ',
        'H' => 'ℍ',
        'N' => 'ℕ',
        'P' => 'ℙ',
        'Q' => 'ℚ',
        'R' => 'ℝ',
        'Z' => 'ℤ',
        'A'..='Z' => char::from_u32(0x1D538 + (c as u32 - 'A' as u32))?,
        'a'..='z' => char::from_u32(0x1D552 + (c as u32 - 'a' as u32))?,
        '0'..='9' => char::from_u32(0x1D7D8 + (c as u32 - '0' as u32))?,
        _ => return None,
    })
}

pub fn script(c: char) -> Option<char> {
    Some(match c {
        'B' => 'ℬ',
        'E' => 'ℰ',
        'F' => 'ℱ',
        'H' => 'ℋ',
        'I' => 'ℐ',
        'L' => 'ℒ',
        'M' => 'ℳ',
        'R' => 'ℛ',
        'e' => 'ℯ',
        'g' => 'ℊ',
        'o' => 'ℴ',
        'A'..='Z' => char::from_u32(0x1D49C + (c as u32 - 'A' as u32))?,
        'a'..='z' => char::from_u32(0x1D4B6 + (c as u32 - 'a' as u32))?,
        _ => return None,
    })
}

/// Back from a double-struck or script character to the letter and the command that makes it.
pub fn unstyle(c: char) -> Option<(&'static str, char)> {
    for l in ('A'..='Z').chain('a'..='z').chain('0'..='9') {
        if blackboard(l) == Some(c) {
            return Some(("mathbb", l));
        }
    }
    for l in ('A'..='Z').chain('a'..='z') {
        if script(l) == Some(c) {
            return Some(("mathcal", l));
        }
    }
    None
}

/// Mathematical bold (upright) and bold italic letters for MathML, which has no style attribute for them.
pub fn bold(c: char) -> char {
    match c {
        'A'..='Z' => char::from_u32(0x1D400 + (c as u32 - 'A' as u32)).unwrap_or(c),
        'a'..='z' => char::from_u32(0x1D41A + (c as u32 - 'a' as u32)).unwrap_or(c),
        '0'..='9' => char::from_u32(0x1D7CE + (c as u32 - '0' as u32)).unwrap_or(c),
        _ => c,
    }
}

pub fn bold_italic(c: char) -> char {
    match c {
        'A'..='Z' => char::from_u32(0x1D468 + (c as u32 - 'A' as u32)).unwrap_or(c),
        'a'..='z' => char::from_u32(0x1D482 + (c as u32 - 'a' as u32)).unwrap_or(c),
        _ => c,
    }
}

/// An accent character as MathML draws it over or under its base (the combining form does not stretch).
pub fn spacing_accent(c: &str) -> &str {
    match c {
        "\u{302}" => "^",
        "\u{30C}" => "ˇ",
        "\u{303}" => "~",
        "\u{301}" => "´",
        "\u{300}" => "`",
        "\u{307}" => "˙",
        "\u{308}" => "¨",
        "\u{306}" => "˘",
        "\u{304}" => "¯",
        "\u{20D7}" => "→",
        "\u{30A}" => "˚",
        other => other,
    }
}
