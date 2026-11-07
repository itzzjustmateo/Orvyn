import { NextRequest, NextResponse } from "next/server";
import { mkdir, readFile, rename, writeFile, unlink } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import path from "node:path";
import { serverConfig } from "@/lib/server-config";

export const dynamic = "force-dynamic";
let writes = Promise.resolve();

async function filename() {
  const cfg = await serverConfig();
  return path.join(cfg.paths.data, "chats.json");
}
async function load(file: string) {
  try {
    return JSON.parse(await readFile(file, "utf8"));
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw error;
  }
}
export async function GET() {
  try {
    return NextResponse.json(await load(await filename()));
  } catch {
    return NextResponse.json({ error: "Failed to read conversations" }, { status: 500 });
  }
}
export async function PUT(request: NextRequest) {
  const origin = request.headers.get("origin");
  if (origin && new URL(origin).host !== request.headers.get("host")) {
    return NextResponse.json({ error: "Invalid origin" }, { status: 403 });
  }
  try {
    const text = await request.text();
    if (Buffer.byteLength(text) > 40_000_000) return NextResponse.json({ error: "Te groot" }, { status: 413 });
    const state = JSON.parse(text);
    if (!Array.isArray(state.chats) || !state.chats.every((c: {id?: unknown; messages?: unknown}) =>
      typeof c.id === "string" && Array.isArray(c.messages))) {
      return NextResponse.json({ error: "Invalid conversations" }, { status: 400 });
    }
    let result = NextResponse.json({});
    const operation = writes.then(async () => {
      const file = await filename();
      const current = await load(file);
      if ((current?.revision ?? null) !== (state.revision ?? null)) {
        result = NextResponse.json({ error: "Conversations changed or were restored elsewhere. Reload this page." }, { status: 409 });
        return;
      }
      const revision = randomUUID();
      await mkdir(path.dirname(file), { recursive: true });
      const temp = `${file}.${revision}.tmp`;
      try {
        await writeFile(temp, JSON.stringify({ ...state, revision }), "utf8");
        await rename(temp, file);
      } finally {
        await unlink(temp).catch(() => {});
      }
      result = NextResponse.json({ revision });
    });
    writes = operation.catch(() => {});
    await operation;
    return result;
  } catch {
    return NextResponse.json({ error: "Failed to save conversations" }, { status: 500 });
  }
}
