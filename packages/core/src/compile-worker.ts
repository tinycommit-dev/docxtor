// Compiles the engine off the page: the page gets the compiled module and none of the memory the compiling used. Compiling the
// 15 MB engine on the page leaves about 27 MB of it behind for the browser to give back up to half a minute later (an empty page
// goes from 20 to 71 MB and settles at 44); compiled here it is gone with the worker, and the page is at 44 at once.
const scope = self as unknown as { postMessage(message: WebAssembly.Module | null): void; onmessage: ((e: MessageEvent<string>) => void) | null };

scope.onmessage = async (e) => {
  try {
    let module: WebAssembly.Module;
    try {
      module = await WebAssembly.compileStreaming(fetch(e.data));
    } catch {
      // A server that labels the file otherwise than application/wasm: compile the bytes.
      module = await WebAssembly.compile(await (await fetch(e.data)).arrayBuffer());
    }
    scope.postMessage(module);
  } catch {
    scope.postMessage(null);
  }
};
