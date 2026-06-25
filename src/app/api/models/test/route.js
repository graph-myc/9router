import { NextResponse } from "next/server";
import { pingModelByKind } from "./ping";

// POST /api/models/test - Ping a single model via internal completions or embeddings
export async function POST(request) {
  try {
    const { model, kind, verbose } = await request.json();
    if (!model) return NextResponse.json({ error: "Model required" }, { status: 400 });
    // verbose (manual single-model test) → request real tokens so reasoning + a full
    // reply come back; the cheap reachability ping (one-by-one) stays at 1 token.
    const result = await pingModelByKind(model, kind || "llm", undefined, { maxTokens: verbose ? 512 : 1 });
    return NextResponse.json(result);
  } catch (err) {
    return NextResponse.json({ ok: false, error: err.message }, { status: 500 });
  }
}
