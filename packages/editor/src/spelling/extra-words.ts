// Everyday words the Hunspell dictionaries Docxtor ships for English were made before: chatbot, pushback, roadmap.
// Each word can name a word the dictionary has, whose endings it takes (chatbot like robot: chatbots, chatbot's).
// Real mistakes (everytime) and names (product names) stay out: Add to dictionary is for those.

/** [word, a known word whose endings it shares]. */
export type ExtraWord = readonly [string, string?];

const ENGLISH: ExtraWord[] = [
  ["chatbot", "robot"],
  ["pushback", "setback"],
  ["cowork", "work"],
  ["coworking"],
  ["signup", "setup"],
  ["livestream", "stream"],
  ["livestreaming"],
  ["fintech"],
  ["onboard", "board"],
  ["onboarding"],
  ["offboard", "board"],
  ["offboarding"],
  ["upskill", "skill"],
  ["reskill", "skill"],
  ["touchpoint", "point"],
  ["roadmap", "map"],
  ["scalable"],
  ["podcaster", "broadcaster"],
  ["vlog", "blog"],
  ["vlogger", "blogger"],
  ["codebase", "base"],
  ["frontend"],
  ["backend"],
  ["devops"],
  ["microservice", "service"],
  ["repo", "memo"],
  ["ebook", "book"],
  ["upvote", "vote"],
  ["downvote", "vote"],
  ["influencer"],
  ["influencers"],
  ["skillset", "set"],
  ["walkthrough"],
  ["walkthroughs"],
  ["standup", "setup"],
  ["ecommerce"],
  ["autocomplete", "complete"],
  ["autocorrect", "correct"],
  ["geolocation", "location"],
  ["dropdown", "breakdown"],
  ["unfollow", "follow"],
  ["agentic"],
  ["subagent", "agent"],
  ["gamification"],
  ["gamify", "magnify"],
  ["upcycle", "cycle"],
  ["rideshare", "share"],
  ["telehealth"],
  ["cyber"],
  ["cybersecurity"],
  ["toolkit", "kit"],
  ["wearables"],
  ["preorder", "order"],
  ["photobomb", "bomb"],
  ["microblog", "blog"],
];

/** The extra words for a language tag (`en`, `en-GB`, `en_US`); none for other languages. */
export function extraWords(lang: string): ExtraWord[] {
  return /^en\b|^en[-_]/i.test(lang) ? ENGLISH : [];
}
