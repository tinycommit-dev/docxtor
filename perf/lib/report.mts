// perf/report.md: plain English, rewritten on every run.
import { fmt, type Verdict } from "./gate.mts";
import type { Load } from "./env.mts";

export interface Goal {
  id: string;
  text: string;
  metric: string;
  max: number;
  unit: string;
  headline?: boolean;
}
export interface ReportInput {
  tier: string;
  commit: string;
  date: string;
  seconds: number;
  load: Load;
  loadOk: boolean;
  verdicts: Verdict[];
  reported: Record<string, number | string>;
  goals: Goal[];
  values: Record<string, number>;
  notes: string[];
  baselineCommit?: string;
  sections: { title: string; body: string }[];
}

export function report(r: ReportInput): string {
  const worse = r.verdicts.filter((v) => v.status === "worse");
  const better = r.verdicts.filter((v) => v.status === "better");
  const fresh = r.verdicts.filter((v) => v.status === "new");
  const ok = r.verdicts.length - worse.length - better.length - fresh.length;
  const l: string[] = [];
  l.push(`# Docxtor performance report`, ``);
  l.push(`Tier **${r.tier}**, commit ${r.commit}, ${r.date}, took ${r.seconds.toFixed(0)} s.`, ``);
  l.push(!r.baselineCommit ? `## No baseline yet: nothing was gated` : worse.length ? `## FAILED: ${worse.length} gate${worse.length > 1 ? "s" : ""} worse than the baseline` : `## Passed: no gate is worse than the baseline`, ``);
  l.push(`${r.verdicts.length} exact counts checked against the baseline${r.baselineCommit ? ` (accepted at ${r.baselineCommit})` : " (there is no baseline yet: run `npm run perf:accept`)"}: ${ok} unchanged, ${better.length} better, ${worse.length} worse, ${fresh.length} new.`, ``);
  if (worse.length) {
    l.push(`| What | Baseline | Now | Allowed | Change |`, `|---|---|---|---|---|`);
    for (const v of worse) l.push(`| ${v.metric} | ${fmt(v.base)} | ${fmt(v.now)} | +${fmt(v.tol)} | +${((v.delta / (v.base || 1)) * 100).toFixed(1)}% |`);
    l.push(``);
  }
  if (better.length) {
    l.push(`Better than the baseline (accept a new baseline to keep the gain): ${better.slice(0, 12).map((v) => `${v.metric} ${fmt(v.base)} to ${fmt(v.now)}`).join("; ")}${better.length > 12 ? `; and ${better.length - 12} more` : ""}.`, ``);
  }
  l.push(`## The machine`, ``);
  l.push(`Load average ${r.load.load1} (1 min), ${r.load.load5} (5 min) on ${r.load.cpus} cores: ${(r.load.perCore * 100).toFixed(0)}% of the cores busy. ${r.load.memoryFreePercent !== null ? `Free memory ${r.load.memoryFreePercent}%. ` : ""}${r.load.busiest ? `Busiest process: ${r.load.busiest}. ` : ""}${r.load.platform}, Node ${r.load.node}.`, ``);
  l.push(r.loadOk ? `The machine was quiet enough to trust times and memory.` : `**The machine was busy. Times and memory below are reported but were NOT used for any decision, and may be tens of percent off. Counts are not affected by load.**`, ``);
  for (const s of r.sections) l.push(`## ${s.title}`, ``, s.body, ``);
  const goals = r.goals.filter((g) => g.metric in r.values || true);
  l.push(`## Goals (reported, never gated)`, ``, `| Goal | Target | Now | |`, `|---|---|---|---|`);
  for (const g of goals) {
    const v = r.values[g.metric];
    l.push(`| ${g.headline ? "**" : ""}${g.text}${g.headline ? "**" : ""} | under ${fmt(g.max)} ${g.unit} | ${v === undefined ? "not measured in this tier" : fmt(v) + " " + g.unit} | ${v === undefined ? "" : v <= g.max ? "met" : "not met"} |`);
  }
  l.push(``);
  if (r.notes.length) l.push(`## Notes`, ``, ...r.notes.map((n) => `- ${n}`), ``);
  return l.join("\n");
}
