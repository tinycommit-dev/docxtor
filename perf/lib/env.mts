// What the machine was doing while a measurement ran. Times and memory move with it; counts do not.
import { execSync } from "node:child_process";
import os from "node:os";

export interface Load {
  load1: number;
  load5: number;
  cpus: number;
  /** load1 divided by the number of cores: 1.0 means every core busy. */
  perCore: number;
  /** macOS: the system's own "memory pressure" (free percentage; lower is worse). Null elsewhere. */
  memoryFreePercent: number | null;
  /** The busiest other process (name and CPU percent), so a stuck process is named in the report. */
  busiest: string | null;
  platform: string;
  node: string;
}

/** Above this share of the cores busy, times and memory are not gated (they are still reported, marked). */
export const MAX_LOAD_PER_CORE = 0.5;

export function machine(): Load {
  const [load1, load5] = os.loadavg();
  const cpus = os.cpus().length;
  let memoryFreePercent: number | null = null;
  let busiest: string | null = null;
  try {
    if (process.platform === "darwin") {
      const out = execSync("memory_pressure", { encoding: "utf8", timeout: 5000 });
      const m = /free percentage:\s*(\d+)%/.exec(out);
      if (m) memoryFreePercent = Number(m[1]);
    }
  } catch {
    /* not available */
  }
  try {
    const out = execSync("ps -Ao pcpu=,comm= -r | head -3", { encoding: "utf8", timeout: 5000 }).trim().split("\n")[0].trim();
    const [, cpu, comm] = /^(\S+)\s+(.*)$/.exec(out) ?? [];
    if (comm) busiest = `${cpu} % ${comm.split("/").pop()}`.slice(0, 80); // the program's name only, never a path under the home folder
  } catch {
    /* not available */
  }
  return { load1: +load1.toFixed(2), load5: +load5.toFixed(2), cpus, perCore: +(load1 / cpus).toFixed(2), memoryFreePercent, busiest, platform: `${process.platform} ${os.release()}`, node: process.version };
}

export const quiet = (l: Load) => l.perCore <= MAX_LOAD_PER_CORE;

export function gitHead(): string {
  try {
    return execSync("git rev-parse --short HEAD", { encoding: "utf8" }).trim();
  } catch {
    return "unknown";
  }
}
