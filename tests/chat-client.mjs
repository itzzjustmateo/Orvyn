import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { transform } from '../apps/web-ui/node_modules/esbuild/lib/main.js';
const source = await readFile(new URL('../apps/web-ui/src/lib/chat-client.ts', import.meta.url), 'utf8');
const {code} = await transform(source, {loader:'ts', format:'esm'});
const {chatRequest} = await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'));
const chunk = text => new TextEncoder().encode(text);
async function withStream(run, options = {}) {
  const original = globalThis.fetch;
  let cancelled = false;
  const stream = new ReadableStream({
    start(controller) { run(controller); },
    cancel() { cancelled = true; },
  });
  globalThis.fetch = async () => new Response(stream, {headers:{'Content-Type':'text/event-stream'}});
  try { return await options.check(() => chatRequest({},new AbortController().signal), () => cancelled); }
  finally { globalThis.fetch = original; }
}
test('DONE completes without waiting for connection close and cancels reader', async () => {
  await withStream(c => c.enqueue(chunk('data: {"choices":[{"delta":{"content":"Klaar"}}]}\n\ndata: [DONE]\n\n')), {
    check: async (request, cancelled) => {
      let timer;
      try {
        const result = await Promise.race([request(), new Promise((_,reject) => {timer=setTimeout(()=>reject(new Error('Client waits after DONE')),500);})]);
        assert.equal(result.choices[0].message.content,'Klaar');
        assert.equal(cancelled(),true);
      } finally { clearTimeout(timer); }
    },
  });
});
test('events after DONE are ignored', async () => {
  await withStream(c => {c.enqueue(chunk('data: [DONE]\n\ndata: invalid-json\n\n')); c.close();}, {
    check: async request => assert.equal((await request()).choices[0].message.content,''),
  });
});
test('UTF-8 characters and events survive chunk boundaries', async () => {
  await withStream(c => {
    for (const byte of chunk('data: {"choices":[{"delta":{"content":"hé 😀"}}]}\r\n\r\ndata: [DONE]\r\n\r\n')) c.enqueue(Uint8Array.of(byte));
    c.close();
  }, {check: async request => assert.equal((await request()).choices[0].message.content,'hé 😀')});
});
test('truncated stream is rejected', async () => {
  await withStream(c => c.close(), {check: async request => assert.rejects(request,/interrupted/)});
});
test('model errors cancel the body', async () => {
  await withStream(c => c.enqueue(chunk('data: {"error":{"message":"model failed"}}\n\n')), {
    check: async (request,cancelled) => {await assert.rejects(request,/model failed/); assert.equal(cancelled(),true);},
  });
});