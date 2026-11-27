import { defineExtension, msg, type Extension } from "@docxtor/extensions";
import { attachContextMenu } from "../ui/context-menu";
import type { Internals } from "./index";

export function contextMenu({ fe, editor, ask, hideBubble, contextSources }: Internals): Extension {
  return defineExtension({
    id: "docxtor.context-menu",
    name: msg("Right-click menu"),
    description: msg("Clipboard, links, formatting and table actions on right-click. Extensions can add to it."),
    activate(ctx) {
      ctx.subscriptions.push({ dispose: attachContextMenu({ editor, host: fe.extensions, ask, onOpen: hideBubble, leading: () => contextSources }) });
    },
  });
}
