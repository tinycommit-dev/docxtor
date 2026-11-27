// The save queue: one write at a time, newest wins, and "saved" means the written revision is the revision now. Runs in Node.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { SaveQueue, type SaveStatus, type Snapshot } from "../src/save-queue.ts";

interface Snap extends Snapshot {
  text: string;
}

/** A document that is a string with a revision, and a write that finishes when the test says so. */
function setup(opts: { fail?: () => boolean } = {}) {
  const doc = { text: "a", revision: 0 };
  const written: Snap[] = [];
  const gates: (() => void)[] = [];
  let writes = 0;
  let active = 0;
  let maxActive = 0;
  const states: SaveStatus[] = [];
  const q = new SaveQueue<Snap>(
    {
      revision: () => doc.revision,
      take: () => ({ revision: doc.revision, text: doc.text }),
      write: (s) => {
        writes++;
        active++;
        maxActive = Math.max(maxActive, active);
        return new Promise<void>((resolve, reject) => {
          gates.push(() => {
            active--;
            if (opts.fail?.()) reject(new Error("disk full"));
            else {
              written.push(s);
              resolve();
            }
          });
        });
      },
      onState: (s) => states.push(s),
    },
    0,
  );
  const edit = (text: string) => {
    doc.text = text;
    doc.revision++;
    q.touch();
  };
  const finish = async () => {
    gates.shift()!();
    await new Promise((r) => setTimeout(r, 0));
  };
  return { doc, q, written, edit, finish, states, writes: () => writes, maxActive: () => maxActive, gates };
}

test("a document that has not changed is saved and is not written", async () => {
  const s = setup();
  assert.equal(s.q.dirty, false);
  assert.equal(s.q.status, "saved");
  const r = await s.q.save();
  assert.equal(r.ok, true);
  assert.equal(s.writes(), 0);
  // An explicit save writes anyway.
  const p = s.q.save(true);
  await s.finish();
  assert.equal((await p).snapshot?.text, "a");
  assert.equal(s.writes(), 1);
});

test("an edit made while a write runs is written next, and the document is unsaved until then", async () => {
  const s = setup();
  s.edit("b");
  const first = s.q.save();
  assert.equal(s.q.status, "saving");
  s.edit("c"); // during the write
  assert.equal(s.q.status, "saving");
  const second = s.q.save(); // the autosave timer
  await s.finish();
  assert.equal((await first).snapshot?.text, "b");
  assert.equal(s.q.dirty, true, "the write held revision 1, the document is at 2");
  await new Promise((r) => setTimeout(r, 0));
  assert.equal(s.gates.length, 1, "the second write has started");
  await s.finish();
  assert.equal((await second).snapshot?.text, "c");
  assert.equal(s.q.dirty, false);
  assert.equal(s.q.status, "saved");
  assert.deepEqual(
    s.written.map((w) => w.text),
    ["b", "c"],
  );
  assert.equal(s.maxActive(), 1, "never two writes at once");
});

test("when the edit during a write is not followed by a request, the state says unsaved", async () => {
  const s = setup();
  s.edit("b");
  void s.q.save();
  s.edit("c");
  await s.finish();
  assert.equal(s.q.status, "unsaved");
  assert.equal(s.states.at(-1), "unsaved");
});

test("many requests during a write make one more write, with the newest text", async () => {
  const s = setup();
  s.edit("b");
  const a = s.q.save();
  s.edit("c");
  const b = s.q.save();
  s.edit("d");
  const c = s.q.save();
  s.edit("e");
  const d = s.q.save();
  await s.finish();
  await new Promise((r) => setTimeout(r, 0));
  assert.equal(s.gates.length, 1);
  await s.finish();
  const results = await Promise.all([a, b, c, d]);
  assert.equal(s.writes(), 2);
  assert.equal(results[0].snapshot?.text, "b");
  for (const r of results.slice(1)) assert.equal(r.snapshot?.text, "e");
  assert.equal(s.q.dirty, false);
});

test("a request that the running write already covers does not write again", async () => {
  const s = setup();
  s.edit("b");
  const a = s.q.save();
  const b = s.q.save(); // nothing changed since the write took its bytes
  await s.finish();
  assert.equal((await a).ok, true);
  assert.equal((await b).ok, true);
  assert.equal(s.writes(), 1);
  // Unless it insists (Ctrl+S).
  const c = s.q.save(true);
  const d = s.q.save(true);
  await s.finish();
  await new Promise((r) => setTimeout(r, 0));
  await s.finish();
  await Promise.all([c, d]);
  assert.equal(s.writes(), 3);
});

test("a failed write leaves the document unsaved, says so, and the next write works", async () => {
  let fail = true;
  const s = setup({ fail: () => fail });
  s.edit("b");
  const p = s.q.save();
  await s.finish();
  const r = await p;
  assert.equal(r.ok, false);
  assert.equal(r.error?.message, "disk full");
  assert.equal(s.q.dirty, true);
  assert.equal(s.q.failed, true);
  assert.equal(s.q.status, "failed");
  assert.equal(s.states.at(-1), "failed");
  fail = false;
  const again = s.q.save();
  await s.finish();
  assert.equal((await again).ok, true);
  assert.equal(s.q.failed, false);
  assert.equal(s.q.status, "saved");
});

test("undo counts as a change: going back to the text that was saved is still unsaved", async () => {
  const s = setup();
  s.edit("b");
  const p = s.q.save();
  await s.finish();
  await p;
  s.edit("a"); // an undo raises the revision as well
  assert.equal(s.q.dirty, true);
});

test("markUnsaved and markSaved", async () => {
  const s = setup();
  s.q.markUnsaved();
  assert.equal(s.q.dirty, true);
  assert.equal(s.q.status, "unsaved");
  s.q.markSaved(s.doc.revision);
  assert.equal(s.q.dirty, false);
  assert.equal(s.q.status, "saved");
});

test("a closed queue finishes the running write, drops the waiting one and never asks the document again", async () => {
  const s = setup();
  s.edit("b");
  const a = s.q.save();
  s.edit("c");
  const b = s.q.save();
  s.q.close();
  s.doc.revision = -5; // a document that is gone
  await s.finish();
  assert.equal((await a).ok, true);
  assert.equal((await b).ok, false);
  assert.equal(s.writes(), 1);
  assert.equal(s.q.dirty, false);
  assert.equal((await s.q.save(true)).ok, false);
  await s.q.idle();
});

test("idle waits for the writes", async () => {
  const s = setup();
  s.edit("b");
  void s.q.save();
  let done = false;
  void s.q.idle().then(() => (done = true));
  await new Promise((r) => setTimeout(r, 0));
  assert.equal(done, false);
  await s.finish();
  await new Promise((r) => setTimeout(r, 0));
  assert.equal(done, true);
});

test("a take that goes on in steps: an edit made meanwhile leaves the document unsaved, and the bytes are written once", async () => {
  const doc = { text: "a", revision: 0 };
  const written: { revision: number; text: string }[] = [];
  let release!: () => void;
  const q = new SaveQueue<Snap>(
    {
      revision: () => doc.revision,
      // The revision is the one the take began at; the text is read after a pause, so it may hold a later edit.
      take: async () => {
        const revision = doc.revision;
        await new Promise<void>((r) => (release = r));
        return { revision, text: doc.text };
      },
      write: async (s) => void written.push(s),
    },
    0,
  );
  const p = q.save(true);
  assert.equal(q.status, "saving");
  doc.text = "ab";
  doc.revision++;
  q.touch();
  release();
  const r = await p;
  assert.equal(r.ok, true);
  assert.deepEqual(written, [{ revision: 0, text: "ab" }]);
  assert.equal(q.dirty, true, "the edit made during the take is not known to be saved");
  assert.equal(q.status, "unsaved");
});
