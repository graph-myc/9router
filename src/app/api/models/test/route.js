import { NextResponse } from "next/server";
import { pingModelByKind } from "./ping";

// POST /api/models/test - Ping a single model via internal completions or embeddings
export async function POST(request) {
  try {
    const { model, kind, verbose } = await request.json();
    if (!model) return NextResponse.json({ error: "Model required" }, { status: 400 });
    // verbose (manual single-model test) → request a few real tokens + return content.
    const result = await pingModelByKind(model, kind || "llm", undefined, { maxTokens: verbose ? 64 : 1 });
    return NextResponse.json(result);
  } catch (err) {
    return NextResponse.json({ ok: false, error: err.message }, { status: 500 });
  }
}
