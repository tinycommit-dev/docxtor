// Example extension: a word-count goal in the navigator, remembered between sessions.
// Shows panels, settings (ctx.state), document events and a prompt.
import { defineExtension } from "@docxtor/extensions";

export default defineExtension({
  id: "example.word-goal",
  name: "Word goal",
  version: "1.0.0",
  publisher: "Docxtor examples",
  description: "Set a word count to aim for and watch your progress.",
  activate(ctx) {
    let goal = ctx.state.get("goal", 1000);
    let view: { bar: HTMLProgressElement; label: HTMLElement } | null = null;
    let timer = 0;

    const words = () => (ctx.document.engine ? ctx.document.text().split(/\s+/).filter(Boolean).length : 0);
    const paint = () => {
      if (!view) return;
      const n = words();
      view.bar.max = goal;
      view.bar.value = Math.min(n, goal);
      view.label.textContent = n >= goal ? `${n.toLocaleString()} words. Goal reached.` : `${n.toLocaleString()} of ${goal.toLocaleString()} words`;
    };
    // Counting reads the whole document, so wait for a pause in typing.
    const later = () => {
      clearTimeout(timer);
      timer = window.setTimeout(paint, 400);
    };

    ctx.panels.register({
      id: "wordGoal",
      title: "Goal",
      order: 40,
      render(el) {
        const label = document.createElement("p");
        const bar = document.createElement("progress");
        bar.style.width = "100%";
        bar.setAttribute("aria-label", "Progress toward your word goal");
        const change = document.createElement("button");
        change.className = "btn";
        change.type = "button";
        change.textContent = "Change goal";
        change.onclick = () => void ctx.commands.execute("wordGoal.set");
        const box = document.createElement("div");
        box.style.cssText = "display:flex;flex-direction:column;gap:8px;padding:8px";
        box.append(label, bar, change);
        el.append(box);
        view = { bar, label };
        paint();
        return () => {
          view = null;
        };
      },
      onShow: paint,
    });

    ctx.commands.register(
      "wordGoal.set",
      async () => {
        const v = await ctx.ui.prompt("Word goal", { value: String(goal), placeholder: "Words", confirm: "Set goal" });
        const n = Math.round(Number(v));
        if (!v || !(n > 0)) return;
        goal = n;
        ctx.state.set("goal", goal);
        paint();
      },
      { title: "Set a word goal…", category: "View" },
    );

    ctx.subscriptions.push(ctx.document.onDidChange(later), ctx.document.onDidOpen(paint), { dispose: () => clearTimeout(timer) });
  },
});
