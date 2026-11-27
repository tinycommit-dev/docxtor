// avoid-ai-design-ignore-file: I2, I3, SD4
// (Emoji, arrows and the middle dot are the data this dialog offers, not chrome.)
// The characters of Insert, Special characters and Emoji: each with an English name for search and for its
// tooltip. Names are data, not interface text, so they are not translated. This module is loaded when the
// dialog opens (a dynamic import), so the editor's main bundle does not carry it.
import { msg } from "@docxtor/extensions";

export interface Glyph {
  /** The character (or emoji sequence). */
  c: string;
  /** Its English name. */
  n: string;
}

export interface Group {
  /** Title (an English key for `t()`), shown above the group's characters; empty for a category with one group. */
  title: string;
  items: Glyph[];
}

export interface Category {
  id: string;
  /** English key for `t()`. */
  title: string;
  groups: Group[];
}

/** One glyph per line: the character, a space, then its name. */
const parse = (s: string): Glyph[] =>
  s
    .trim()
    .split("\n")
    .map((line) => {
      const i = line.indexOf(" ");
      return { c: line.slice(0, i), n: line.slice(i + 1).trim() };
    });

const one = (id: string, title: string, items: Glyph[]): Category => ({ id, title, groups: [{ title: "", items }] });

const MIDDLE_DOT: Glyph = { c: "·", n: "middle dot" };

const COMMON = parse(`
© copyright sign
® registered sign
™ trade mark sign
℠ service mark
§ section sign
¶ pilcrow sign
† dagger
‡ double dagger
• bullet
… horizontal ellipsis
– en dash
— em dash
‘ left single quotation mark
’ right single quotation mark
“ left double quotation mark
” right double quotation mark
« left-pointing double angle quotation mark
» right-pointing double angle quotation mark
° degree sign
± plus-minus sign
× multiplication sign
÷ division sign
≠ not equal to
≈ almost equal to
≤ less-than or equal to
≥ greater-than or equal to
∞ infinity
√ square root
½ vulgar fraction one half
¼ vulgar fraction one quarter
¾ vulgar fraction three quarters
‰ per mille sign
€ euro sign
£ pound sign
¥ yen sign
¢ cent sign
µ micro sign
¿ inverted question mark
¡ inverted exclamation mark
№ numero sign
′ prime
″ double prime
℃ degree celsius
℉ degree fahrenheit
✓ check mark
★ black star
→ rightwards arrow
← leftwards arrow
`);

const ARROWS = parse(`
← leftwards arrow
→ rightwards arrow
↑ upwards arrow
↓ downwards arrow
↔ left right arrow
↕ up down arrow
↖ north west arrow
↗ north east arrow
↘ south east arrow
↙ south west arrow
⇐ leftwards double arrow
⇒ rightwards double arrow
⇑ upwards double arrow
⇓ downwards double arrow
⇔ left right double arrow
⇕ up down double arrow
⇄ rightwards arrow over leftwards arrow
⇆ leftwards arrow over rightwards arrow
↩ leftwards arrow with hook
↪ rightwards arrow with hook
↰ upwards arrow with tip leftwards
↱ upwards arrow with tip rightwards
↲ downwards arrow with tip leftwards
↳ downwards arrow with tip rightwards
↵ downwards arrow with corner leftwards (return)
↺ anticlockwise open circle arrow
↻ clockwise open circle arrow
↦ rightwards arrow from bar (maps to)
↤ leftwards arrow from bar
⟵ long leftwards arrow
⟶ long rightwards arrow
⟷ long left right arrow
⟸ long leftwards double arrow
⟹ long rightwards double arrow
⟺ long left right double arrow
⇦ leftwards white arrow
⇨ rightwards white arrow
⇧ upwards white arrow
⇩ downwards white arrow
➔ heavy wide-headed rightwards arrow
➜ heavy round-tipped rightwards arrow
➤ black rightwards arrowhead
➡ black rightwards arrow
⬅ leftwards black arrow
⬆ upwards black arrow
⬇ downwards black arrow
⤴ arrow pointing rightwards then curving upwards
⤵ arrow pointing rightwards then curving downwards
▲ black up-pointing triangle
▼ black down-pointing triangle
◀ black left-pointing triangle
▶ black right-pointing triangle
`);

const MATH = parse(`
+ plus sign
− minus sign
± plus-minus sign
∓ minus-or-plus sign
× multiplication sign
÷ division sign
⋅ dot operator
∗ asterisk operator
= equals sign
≠ not equal to
≈ almost equal to
≡ identical to
≢ not identical to
≅ approximately equal to
∼ tilde operator
∝ proportional to
< less-than sign
> greater-than sign
≤ less-than or equal to
≥ greater-than or equal to
≪ much less-than
≫ much greater-than
∞ infinity
√ square root
∛ cube root
∜ fourth root
∑ n-ary summation
∏ n-ary product
∫ integral
∬ double integral
∮ contour integral
∂ partial differential
∇ nabla
∆ increment
∈ element of
∉ not an element of
∋ contains as member
∅ empty set
⊂ subset of
⊃ superset of
⊆ subset of or equal to
⊇ superset of or equal to
∪ union
∩ intersection
∧ logical and
∨ logical or
¬ not sign
∀ for all
∃ there exists
∄ there does not exist
∴ therefore
∵ because
⊥ up tack (perpendicular)
∥ parallel to
∠ angle
° degree sign
′ prime
″ double prime
⌊ left floor
⌋ right floor
⌈ left ceiling
⌉ right ceiling
‰ per mille sign
% percent sign
½ vulgar fraction one half
⅓ vulgar fraction one third
⅔ vulgar fraction two thirds
¼ vulgar fraction one quarter
¾ vulgar fraction three quarters
⅛ vulgar fraction one eighth
⅜ vulgar fraction three eighths
⅝ vulgar fraction five eighths
⅞ vulgar fraction seven eighths
¹ superscript one
² superscript two
³ superscript three
⁴ superscript four
ⁿ superscript latin small letter n
₀ subscript zero
₁ subscript one
₂ subscript two
₃ subscript three
ℕ double-struck capital n (natural numbers)
ℤ double-struck capital z (integers)
ℚ double-struck capital q (rational numbers)
ℝ double-struck capital r (real numbers)
ℂ double-struck capital c (complex numbers)
∘ ring operator
⊕ circled plus
⊗ circled times
⋯ midline horizontal ellipsis
⋮ vertical ellipsis
`);

const CURRENCY = parse(`
$ dollar sign
€ euro sign
£ pound sign
¥ yen sign
¢ cent sign
₹ indian rupee sign
₽ russian ruble sign
₩ won sign
₪ new sheqel sign
₫ dong sign
₺ turkish lira sign
₴ hryvnia sign
₦ naira sign
₱ philippine peso sign
฿ thai baht sign
₡ colon sign
₲ guarani sign
₵ cedi sign
₸ tenge sign
₼ manat sign
₾ lari sign
₿ bitcoin sign
¤ currency sign
ƒ latin small letter f with hook (florin)
₠ euro-currency sign
₣ french franc sign
₤ lira sign
₧ peseta sign
₭ kip sign
₮ tugrik sign
₨ rupee sign
₳ austral sign
`);

const ACCENTS: Record<string, string> = {
  "̀": "grave",
  "́": "acute",
  "̂": "circumflex",
  "̃": "tilde",
  "̄": "macron",
  "̆": "breve",
  "̇": "dot above",
  "̈": "diaeresis",
  "̊": "ring above",
  "̋": "double acute",
  "̌": "caron",
  "̧": "cedilla",
  "̨": "ogonek",
};

const LETTERS: Record<string, string> = {
  Æ: "Latin capital letter AE",
  æ: "Latin small letter ae",
  Ð: "Latin capital letter eth",
  ð: "Latin small letter eth",
  Þ: "Latin capital letter thorn",
  þ: "Latin small letter thorn",
  ß: "Latin small letter sharp s",
  Ø: "Latin capital letter O with stroke",
  ø: "Latin small letter o with stroke",
  Œ: "Latin capital ligature OE",
  œ: "Latin small ligature oe",
  Ł: "Latin capital letter L with stroke",
  ł: "Latin small letter l with stroke",
  Đ: "Latin capital letter D with stroke",
  đ: "Latin small letter d with stroke",
  Ħ: "Latin capital letter H with stroke",
  ħ: "Latin small letter h with stroke",
  ı: "Latin small letter dotless i",
};

function latinName(c: string): string {
  if (LETTERS[c]) return LETTERS[c];
  const d = c.normalize("NFD");
  const accent = ACCENTS[d.slice(1)];
  const base = d[0];
  return `Latin ${base === base.toUpperCase() ? "capital" : "small"} letter ${base} with ${accent ?? "accent"}`;
}

const LATIN = [...("ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖØÙÚÛÜÝÞß" + "àáâãäåæçèéêëìíîïðñòóôõöøùúûüýþÿ" + "ĀāĂăĄąĆćČčĎďĐđĒēĘęĚěĞğĦħİıŁłŃńŇňŐőŒœŘřŚśŞşŠšŢţŤťŮůŰűŹźŻżŽž")].map((c) => ({ c, n: latinName(c) }));

const GREEK_NAMES = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega".split(" ");
const GREEK: Glyph[] = [];
GREEK_NAMES.forEach((n, i) => GREEK.push({ c: String.fromCodePoint(0x3b1 + i + (i >= 17 ? 1 : 0)), n: `Greek small letter ${n}` }));
GREEK.push({ c: "ς", n: "Greek small letter final sigma" });
GREEK_NAMES.forEach((n, i) => GREEK.push({ c: String.fromCodePoint(0x391 + i + (i >= 17 ? 1 : 0)), n: `Greek capital letter ${n}` }));
GREEK.push({ c: "ϕ", n: "Greek phi symbol" }, { c: "ϑ", n: "Greek theta symbol" }, { c: "ϖ", n: "Greek pi symbol" }, { c: "ϵ", n: "Greek lunate epsilon symbol" });

const SPACES: Glyph[] = [
  { c: " ", n: "no-break space" },
  { c: " ", n: "en space" },
  { c: " ", n: "em space" },
  { c: " ", n: "thin space" },
  { c: " ", n: "narrow no-break space" },
  { c: "‑", n: "non-breaking hyphen" },
];

const PUNCTUATION = [
  ...parse(`
‘ left single quotation mark
’ right single quotation mark
‚ single low-9 quotation mark
“ left double quotation mark
” right double quotation mark
„ double low-9 quotation mark
‹ single left-pointing angle quotation mark
› single right-pointing angle quotation mark
« left-pointing double angle quotation mark
» right-pointing double angle quotation mark
' apostrophe
" quotation mark
¿ inverted question mark
¡ inverted exclamation mark
‽ interrobang
… horizontal ellipsis
‥ two dot leader
• bullet
‣ triangular bullet
⁃ hyphen bullet
– en dash
— em dash
‒ figure dash
― horizontal bar
‐ hyphen
† dagger
‡ double dagger
§ section sign
¶ pilcrow sign
※ reference mark
⁂ asterism
‰ per mille sign
‱ per ten thousand sign
№ numero sign
′ prime
″ double prime
‴ triple prime
¦ broken bar
| vertical line
~ tilde
^ circumflex accent
\` grave accent
´ acute accent
¨ diaeresis
¯ macron
¸ cedilla
ˆ modifier letter circumflex accent
˜ small tilde
@ commercial at
# number sign
& ampersand
* asterisk
ª feminine ordinal indicator
º masculine ordinal indicator
`),
  MIDDLE_DOT,
  ...SPACES,
];

const SYMBOLS = parse(`
✓ check mark
✔ heavy check mark
✗ ballot x
✘ heavy ballot x
✕ multiplication x
✖ heavy multiplication x
☐ ballot box
☑ ballot box with check
☒ ballot box with x
★ black star
☆ white star
✦ black four pointed star
✧ white four pointed star
✪ circled white star
♠ black spade suit
♣ black club suit
♥ black heart suit
♦ black diamond suit
♤ white spade suit
♧ white club suit
♡ white heart suit
♢ white diamond suit
● black circle
○ white circle
◉ fisheye
◆ black diamond
◇ white diamond
■ black square
□ white square
▲ black up-pointing triangle
△ white up-pointing triangle
▼ black down-pointing triangle
▽ white down-pointing triangle
◯ large circle
♪ eighth note
♫ beamed eighth notes
♩ quarter note
♬ beamed sixteenth notes
♭ music flat sign
♮ music natural sign
♯ music sharp sign
☀ black sun with rays
☁ cloud
☂ umbrella
☃ snowman
❄ snowflake
☎ black telephone
✆ telephone location sign
✉ envelope
✂ black scissors
✈ airplane
☮ peace symbol
☯ yin yang
☺ white smiling face
☹ white frowning face
⚠ warning sign
⚑ black flag
⚐ white flag
☢ radioactive sign
☣ biohazard sign
⚡ high voltage sign
❤ heavy black heart
❦ floral heart
❀ white florette
✿ black florette
☘ shamrock
♻ black universal recycling symbol
⚙ gear
⚖ scales
⚓ anchor
♀ female sign
♂ male sign
℮ estimated symbol
© copyright sign
® registered sign
™ trade mark sign
℠ service mark
℗ sound recording copyright
✎ lower right pencil
✐ upper right pencil
☞ white right pointing index
☜ white left pointing index
⌘ place of interest sign (command key)
⌥ option key
⇧ upwards white arrow (shift key)
⎋ broken circle with northwest arrow (escape)
⌫ erase to the left (delete key)
`);

const SMILEYS = parse(`
😀 grinning face
😃 grinning face with big eyes
😄 grinning face with smiling eyes
😁 beaming face with smiling eyes
😆 grinning squinting face
😅 grinning face with sweat
🤣 rolling on the floor laughing
😂 face with tears of joy
🙂 slightly smiling face
🙃 upside-down face
😉 winking face
😊 smiling face with smiling eyes
😇 smiling face with halo
🥰 smiling face with hearts
😍 smiling face with heart-eyes
🤩 star-struck
😘 face blowing a kiss
😋 face savoring food
😛 face with tongue
😜 winking face with tongue
🤪 zany face
🤗 smiling face with open hands (hug)
🤔 thinking face
🤨 face with raised eyebrow
😐 neutral face
😑 expressionless face
😶 face without mouth
🙄 face with rolling eyes
😏 smirking face
😒 unamused face
😬 grimacing face
😌 relieved face
😔 pensive face
😴 sleeping face
🤒 face with thermometer
🤕 face with head-bandage
🤢 nauseated face
🥵 hot face
🥶 cold face
😵 face with crossed-out eyes
🤯 exploding head
🥳 partying face
😎 smiling face with sunglasses
🤓 nerd face
🧐 face with monocle
😕 confused face
😟 worried face
🙁 slightly frowning face
😮 face with open mouth
😲 astonished face
😳 flushed face
🥺 pleading face
😢 crying face
😭 loudly crying face
😱 face screaming in fear
😖 confounded face
😞 disappointed face
😓 downcast face with sweat
😩 weary face
😠 angry face
😡 pouting face
🤬 face with symbols on mouth
💀 skull
💩 pile of poo
🤡 clown face
👻 ghost
👽 alien
🤖 robot
😺 grinning cat
`);

const PEOPLE = parse(`
👍 thumbs up
👎 thumbs down
👌 OK hand
✌️ victory hand
🤞 crossed fingers
🤟 love-you gesture
🤘 sign of the horns
👈 backhand index pointing left
👉 backhand index pointing right
👆 backhand index pointing up
👇 backhand index pointing down
☝️ index pointing up
✋ raised hand
🖐️ hand with fingers splayed
👋 waving hand
🤚 raised back of hand
👏 clapping hands
🙌 raising hands
👐 open hands
🤝 handshake
🙏 folded hands
✍️ writing hand
💪 flexed biceps
🧠 brain
👀 eyes
👁️ eye
👂 ear
👃 nose
👄 mouth
👶 baby
🧒 child
👨 man
👩 woman
🧑 person
👴 old man
👵 old woman
🧑‍💻 technologist
🧑‍🎓 student
🧑‍🏫 teacher
🧑‍🔬 scientist
🙋 person raising hand
🤷 person shrugging
🤦 person facepalming
🙇 person bowing
💁 person tipping hand
🚶 person walking
🏃 person running
💃 woman dancing
🕺 man dancing
`);

const ANIMALS = parse(`
🐶 dog face
🐱 cat face
🐭 mouse face
🐹 hamster
🐰 rabbit face
🦊 fox
🐻 bear
🐼 panda
🐨 koala
🐯 tiger face
🦁 lion
🐮 cow face
🐷 pig face
🐸 frog
🐵 monkey face
🐔 chicken
🐧 penguin
🐦 bird
🦉 owl
🦄 unicorn
🐝 honeybee
🦋 butterfly
🐛 bug
🐌 snail
🐢 turtle
🐍 snake
🐙 octopus
🐬 dolphin
🐳 spouting whale
🦈 shark
🐘 elephant
🐴 horse face
🐑 ewe
🌱 seedling
🌲 evergreen tree
🌳 deciduous tree
🌴 palm tree
🌵 cactus
🌷 tulip
🌹 rose
🌻 sunflower
🍀 four leaf clover
🍁 maple leaf
🍂 fallen leaf
🍄 mushroom
🌍 globe showing Europe-Africa
🌙 crescent moon
⭐ star
🌟 glowing star
✨ sparkles
⚡ high voltage
🔥 fire
🌈 rainbow
☀️ sun
⛅ sun behind cloud
☁️ cloud
🌧️ cloud with rain
❄️ snowflake
💧 droplet
🌊 water wave
`);

const FOOD = parse(`
🍎 red apple
🍐 pear
🍊 tangerine
🍋 lemon
🍌 banana
🍉 watermelon
🍇 grapes
🍓 strawberry
🍒 cherries
🍑 peach
🥭 mango
🍍 pineapple
🥥 coconut
🥝 kiwi fruit
🍅 tomato
🥑 avocado
🥕 carrot
🌽 ear of corn
🥦 broccoli
🥔 potato
🍞 bread
🥐 croissant
🧀 cheese wedge
🥚 egg
🥞 pancakes
🥓 bacon
🍔 hamburger
🍟 french fries
🍕 pizza
🌭 hot dog
🌮 taco
🥗 green salad
🍝 spaghetti
🍜 steaming bowl
🍣 sushi
🍦 soft ice cream
🍩 doughnut
🍪 cookie
🎂 birthday cake
🍰 shortcake
🍫 chocolate bar
🍿 popcorn
☕ hot beverage
🍵 teacup without handle
🍺 beer mug
🍷 wine glass
🥂 clinking glasses
🍾 bottle with popping cork
`);

const ACTIVITIES = parse(`
⚽ soccer ball
🏀 basketball
🏈 american football
⚾ baseball
🎾 tennis
🏐 volleyball
🏉 rugby football
🎱 pool 8 ball
🏓 ping pong
🏸 badminton
🥊 boxing glove
⛳ flag in hole
🎯 bullseye
🎮 video game
🎲 game die
🧩 puzzle piece
🎨 artist palette
🎭 performing arts
🎬 clapper board
🎤 microphone
🎧 headphone
🎵 musical note
🎶 musical notes
🎸 guitar
🎹 musical keyboard
🥇 1st place medal
🏆 trophy
🎉 party popper
🎊 confetti ball
🎈 balloon
🎁 wrapped gift
`);

const TRAVEL = parse(`
🚗 automobile
🚕 taxi
🚌 bus
🚎 trolleybus
🚑 ambulance
🚒 fire engine
🚓 police car
🚚 delivery truck
🚲 bicycle
🛵 motor scooter
🏍️ motorcycle
🚂 locomotive
🚆 train
🚇 metro
🚁 helicopter
✈️ airplane
🚀 rocket
🛸 flying saucer
⛵ sailboat
🚢 ship
⚓ anchor
🚦 vertical traffic light
🗺️ world map
🧭 compass
🏔️ snow-capped mountain
🌋 volcano
🏖️ beach with umbrella
🏝️ desert island
🏠 house
🏢 office building
🏥 hospital
🏫 school
🏛️ classical building
🗼 Tokyo tower
🗽 Statue of Liberty
⛪ church
🕌 mosque
🌅 sunrise
🌃 night with stars
🌆 cityscape at dusk
`);

const OBJECTS = parse(`
⌚ watch
📱 mobile phone
💻 laptop
🖥️ desktop computer
⌨️ keyboard
🖱️ computer mouse
💾 floppy disk
💿 optical disk
📷 camera
📺 television
📻 radio
🔋 battery
🔌 electric plug
💡 light bulb
🔦 flashlight
📚 books
📖 open book
📓 notebook
📒 ledger
📝 memo
✏️ pencil
🖊️ pen
🖍️ crayon
📎 paperclip
📌 pushpin
📍 round pushpin
✂️ scissors
📁 file folder
📂 open file folder
📄 page facing up
📋 clipboard
📅 calendar
📆 tear-off calendar
📊 bar chart
📈 chart increasing
📉 chart decreasing
📧 e-mail
✉️ envelope
📦 package
🔍 magnifying glass tilted left
🔎 magnifying glass tilted right
🔒 locked
🔓 unlocked
🔑 key
🔧 wrench
🔨 hammer
⚙️ gear
💰 money bag
💳 credit card
🏷️ label
🔔 bell
📢 loudspeaker
💬 speech balloon
💭 thought balloon
🎓 graduation cap
🧪 test tube
🔬 microscope
🔭 telescope
`);

const EMOJI_SYMBOLS = parse(`
❤️ red heart
🧡 orange heart
💛 yellow heart
💚 green heart
💙 blue heart
💜 purple heart
🖤 black heart
💔 broken heart
💕 two hearts
💯 hundred points
✅ check mark button
✔️ check mark
❌ cross mark
❎ cross mark button
❓ red question mark
❗ red exclamation mark
‼️ double exclamation mark
⚠️ warning
🚫 prohibited
⛔ no entry
🔴 red circle
🟠 orange circle
🟡 yellow circle
🟢 green circle
🔵 blue circle
🟣 purple circle
⚫ black circle
⚪ white circle
🟥 red square
🟩 green square
🟦 blue square
➕ plus
➖ minus
➗ divide
♻️ recycling symbol
🔁 repeating button
🔀 shuffle tracks button
▶️ play button
⏸️ pause button
⏹️ stop button
🆗 OK button
🆕 NEW button
🔗 link
`);

/** Flags are two regional indicator letters; the country names are the English short names. */
const flag = (code: string, name: string): Glyph => ({ c: [...code].map((ch) => String.fromCodePoint(0x1f1e6 + ch.charCodeAt(0) - 65)).join(""), n: `flag: ${name}` });
const FLAGS: Glyph[] = [
  ["AR", "Argentina"],
  ["AU", "Australia"],
  ["AT", "Austria"],
  ["BE", "Belgium"],
  ["BR", "Brazil"],
  ["CA", "Canada"],
  ["CL", "Chile"],
  ["CN", "China"],
  ["CO", "Colombia"],
  ["DK", "Denmark"],
  ["EG", "Egypt"],
  ["FI", "Finland"],
  ["FR", "France"],
  ["DE", "Germany"],
  ["GR", "Greece"],
  ["IN", "India"],
  ["ID", "Indonesia"],
  ["IE", "Ireland"],
  ["IL", "Israel"],
  ["IT", "Italy"],
  ["JP", "Japan"],
  ["KE", "Kenya"],
  ["MX", "Mexico"],
  ["NL", "Netherlands"],
  ["NZ", "New Zealand"],
  ["NG", "Nigeria"],
  ["NO", "Norway"],
  ["PK", "Pakistan"],
  ["PE", "Peru"],
  ["PL", "Poland"],
  ["PT", "Portugal"],
  ["RU", "Russia"],
  ["SA", "Saudi Arabia"],
  ["SG", "Singapore"],
  ["ZA", "South Africa"],
  ["KR", "South Korea"],
  ["ES", "Spain"],
  ["SE", "Sweden"],
  ["CH", "Switzerland"],
  ["TR", "Turkey"],
  ["UA", "Ukraine"],
  ["AE", "United Arab Emirates"],
  ["GB", "United Kingdom"],
  ["US", "United States"],
  ["UN", "United Nations"],
  ["EU", "European Union"],
].map(([code, name]) => flag(code, name));

export const CATEGORIES: Category[] = [
  one("common", msg("Common"), [...COMMON, MIDDLE_DOT]),
  one("arrows", msg("Arrows"), ARROWS),
  one("math", msg("Math"), MATH),
  one("greek", msg("Greek"), GREEK),
  one("currency", msg("Currency"), CURRENCY),
  one("latin", msg("Latin"), LATIN),
  one("punctuation", msg("Punctuation"), PUNCTUATION),
  one("symbols", msg("Symbols"), SYMBOLS),
  {
    id: "emoji",
    title: msg("Emoji"),
    groups: [
      { title: msg("Smileys"), items: SMILEYS },
      { title: msg("People"), items: PEOPLE },
      { title: msg("Animals and nature"), items: ANIMALS },
      { title: msg("Food"), items: FOOD },
      { title: msg("Activities"), items: ACTIVITIES },
      { title: msg("Travel"), items: TRAVEL },
      { title: msg("Objects"), items: OBJECTS },
      { title: msg("Symbols"), items: EMOJI_SYMBOLS },
      { title: msg("Flags"), items: FLAGS },
    ],
  },
];

/** The code points of a character, as "U+00A9" (several for a sequence: "U+1F1FA U+1F1F8"). */
export function codePoints(c: string): string {
  return [...c].map((ch) => `U+${ch.codePointAt(0)!.toString(16).toUpperCase().padStart(4, "0")}`).join(" ");
}

interface Entry extends Glyph {
  /** Lower-case name and code points, to search. */
  s: string;
}
let index: Entry[] | null = null;

/** Characters whose name has every word typed (or that are the character typed, or have the code point typed). */
export function search(query: string): Glyph[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  if (!index) {
    const seen = new Set<string>();
    index = [];
    for (const cat of CATEGORIES)
      for (const g of cat.groups)
        for (const item of g.items) {
          if (seen.has(item.c)) continue;
          seen.add(item.c);
          index.push({ ...item, s: `${item.n.toLowerCase()} ${codePoints(item.c).toLowerCase()}` });
        }
  }
  const words = q.split(/\s+/);
  const hex = /^u\+?([0-9a-f]{2,6})$/.exec(q)?.[1];
  const out: Entry[] = [];
  for (const e of index) {
    if (e.c === query.trim() || (hex && e.s.includes(`u+${hex.padStart(4, "0")}`)) || words.every((w) => e.s.includes(w))) out.push(e);
  }
  // Names that start with the first word come first, then names with it as a whole word.
  const w = words[0];
  const score = (e: Entry) => (e.n.toLowerCase().startsWith(w) ? 0 : new RegExp(`\\b${w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`).test(e.n.toLowerCase()) ? 1 : 2);
  return out.sort((a, b) => score(a) - score(b)).slice(0, 400);
}
