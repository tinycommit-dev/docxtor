// Every open document has its own address, built from where it lives:
//   #/library/Reports/Q3 plan.docx      a document in the browser Library
//   #/folder/Thesis/chapters/two.docx   a document in a connected folder (by folder name)
//   #/file/notes.docx                   a single file opened from disk (by its Recents entry)
//   #/storage/acme-drive/Specs/a.docx   a document in storage an extension provides (by provider id)
//   #/  (or nothing)                    the start screen
// Hash addresses work on any static host and survive reloads; Back and Forward move between documents.

export type Route =
  | { kind: "home" }
  | { kind: "library"; path: string }
  | { kind: "folder"; folder: string; path: string }
  | { kind: "file"; name: string }
  | { kind: "storage"; provider: string; path: string };

const enc = (path: string) => path.split("/").map(encodeURIComponent).join("/");
const dec = (s: string) => {
  try {
    return decodeURIComponent(s);
  } catch {
    return s;
  }
};

export function routeHash(r: Route): string {
  switch (r.kind) {
    case "home":
      return "#/";
    case "library":
      return `#/library/${enc(r.path)}`;
    case "folder":
      return `#/folder/${encodeURIComponent(r.folder)}/${enc(r.path)}`;
    case "file":
      return `#/file/${encodeURIComponent(r.name)}`;
    case "storage":
      return `#/storage/${encodeURIComponent(r.provider)}/${enc(r.path)}`;
  }
}

export function parseRoute(hash: string): Route {
  const parts = hash.replace(/^#\/?/, "").split("/").filter(Boolean).map(dec);
  const [kind, ...rest] = parts;
  if (kind === "library" && rest.length) return { kind: "library", path: rest.join("/") };
  if (kind === "folder" && rest.length > 1) return { kind: "folder", folder: rest[0], path: rest.slice(1).join("/") };
  if (kind === "file" && rest.length) return { kind: "file", name: rest.join("/") };
  if (kind === "storage" && rest.length > 1) return { kind: "storage", provider: rest[0], path: rest.slice(1).join("/") };
  return { kind: "home" };
}

export function sameRoute(a: Route, b: Route): boolean {
  return routeHash(a) === routeHash(b);
}
