// The engine's compiled WebAssembly module, kept so a worker that needs the engine (the save worker) gets this one and not a second
// compile: a module posted to a worker shares its compiled code with the page, where compiling again costs the worker about 100 MB.
let compiled: WebAssembly.Module | undefined;

export function setEngineModule(m: unknown): void {
  if (typeof WebAssembly !== "undefined" && m instanceof WebAssembly.Module) compiled = m;
}

export function engineModule(): WebAssembly.Module | undefined {
  return compiled;
}
