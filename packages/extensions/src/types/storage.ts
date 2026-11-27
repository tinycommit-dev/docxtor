// Storage providers

export interface StorageEntry {
  kind: "file" | "dir";
  name: string;
  /** Path from the provider's root, `/`-separated, no leading slash. */
  path: string;
  modified?: number;
  size?: number;
}

/** What a storage provider may be told when it writes a document. */
export interface StorageWriteOptions {
  /**
   * The version the writer last read or wrote (see `StorageWriteResult.version`). A provider that keeps versions refuses
   * the write when the stored one is another, by rejecting with an error whose `code` is `"conflict"`, so two people or
   * two devices with the same file open do not overwrite each other without a word. A provider that does not keep
   * versions ignores it.
   */
  expectedVersion?: string;
}

/** What a storage provider may answer a write with. */
export interface StorageWriteResult {
  /** The version the document has now: opaque to Docxtor, handed back as `expectedVersion` on the next write. */
  version?: string;
}

/** A place documents live: a cloud drive, a WebDAV server, a company's document store. */
export interface StorageProvider {
  /** Stable id; part of document addresses and version-history keys. */
  id: string;
  label: string;
  description?: string;
  /** Save edits automatically, like the Library (default true). Turn off where each write is costly. */
  autosave?: boolean;
  list(path: string): Promise<StorageEntry[]>;
  read(path: string): Promise<Uint8Array>;
  write(path: string, bytes: Uint8Array, options?: StorageWriteOptions): Promise<void | StorageWriteResult>;
  mkdir(path: string): Promise<void>;
  /** Rename the entry at `path` to `name` (same folder); returns the new path. */
  rename(path: string, name: string): Promise<string>;
  remove(path: string): Promise<void>;
  exists(path: string): Promise<boolean>;
}

// ---------------------------------------------------------------------------
// Proofing: spelling dictionaries and grammar checkers
// ---------------------------------------------------------------------------

