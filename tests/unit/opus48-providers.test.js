// Regression tests for the Claude Opus 4.8 enhancement on the Claude Code (cc)
// and GitHub Copilot (gh) providers: 1M context window + xhigh/max effort.
import { describe, it, expect } from "vitest";
import { getCapabilitiesForModel } from "../../open-sse/providers/capabilities.js";
import { stripUnsupportedParams } from "../../open-sse/translator/concerns/paramSupport.js";
import { normalizeClaudePassthrough } from "../../open-sse/translator/formats/claude.js";
import { BaseExecutor } from "../../open-sse/executors/base.js";
import { PROVIDERS } from "../../open-sse/providers/index.js";

describe("Opus 4.8 capabilities (cc dash + gh dot ids)", () => {
  for (const [provider, model] of [
    ["claude", "claude-opus-4-8"],
    ["claude", "claude-opus-4-7"],
    ["github", "claude-opus-4.8"],
    ["github", "claude-opus-4.7"],
  ]) {
    it(`${provider}/${model} → adaptive thinking, 1M context, 128K output, maxEffort max`, () => {
      const c = getCapabilitiesForModel(provider, model);
      expect(c.thinkingFormat).toBe("claude-adaptive");
      expect(c.contextWindow).toBe(1000000);
      expect(c.maxOutput).toBe(128000);
      expect(c.maxEffort).toBe("max");
    });
  }

  it("opus-4.6 keeps 1M context but does NOT advertise xhigh/max effort", () => {
    const c = getCapabilitiesForModel("claude", "claude-opus-4-6");
    expect(c.contextWindow).toBe(1000000);
    expect(c.maxEffort).toBe("high");
  });
});

describe("GitHub Copilot reasoning_effort stripping (#713)", () => {
  it("keeps reasoning_effort for Copilot Opus 4.6/4.7/4.8", () => {
    for (const m of ["claude-opus-4.6", "claude-opus-4.7", "claude-opus-4.8", "claude-sonnet-4.6"]) {
      const body = { reasoning_effort: "xhigh", thinking: { type: "enabled" } };
      stripUnsupportedParams("github", m, body);
      expect(body.reasoning_effort, m).toBe("xhigh");
    }
  });

  it("still drops reasoning_effort for older Copilot Claude models", () => {
    const body = { reasoning_effort: "high" };
    stripUnsupportedParams("github", "claude-opus-4.5", body);
    expect(body.reasoning_effort).toBeUndefined();
  });
});

describe("Claude Code passthrough folds provider-effort override into output_config", () => {
  const fold = (model, body) => { const b = { messages: [], ...body }; normalizeClaudePassthrough(b, model); return b; };

  it("reasoning_effort xhigh/max → output_config.effort on Opus 4.8 (native Claude has no reasoning_effort)", () => {
    expect(fold("claude-opus-4-8", { reasoning_effort: "xhigh" })).toMatchObject({ output_config: { effort: "xhigh" } });
    expect(fold("claude-opus-4-8", { reasoning_effort: "max" })).toMatchObject({ output_config: { effort: "max" } });
    expect(fold("claude-opus-4-8", { reasoning_effort: "xhigh" }).reasoning_effort).toBeUndefined();
  });

  it("clamps xhigh down to high on Opus 4.6 (no xhigh support)", () => {
    expect(fold("claude-opus-4-6", { reasoning_effort: "xhigh" })).toMatchObject({ output_config: { effort: "high" } });
  });

  it("none/off → disable thinking", () => {
    expect(fold("claude-opus-4-8", { reasoning_effort: "none" })).toMatchObject({ thinking: { type: "disabled" } });
  });

  it("does not override an effort the client already set", () => {
    const out = fold("claude-opus-4-8", { reasoning_effort: "low", output_config: { effort: "xhigh" } });
    expect(out.output_config.effort).toBe("xhigh");
    expect(out.reasoning_effort).toBeUndefined();
  });
});

describe("Per-model 1M context opt-in appends context-1m beta header", () => {
  const ex = new BaseExecutor("claude", PROVIDERS.claude);
  const beta = (h) => Object.entries(h).find(([k]) => k.toLowerCase() === "anthropic-beta")?.[1] || "";

  it("appends context-1m-2025-08-07 when credentials.context1m is set", () => {
    expect(beta(ex.buildHeaders({ accessToken: "sk-ant-oat-x", context1m: true }))).toContain("context-1m-2025-08-07");
  });

  it("leaves the beta header untouched when context1m is off", () => {
    expect(beta(ex.buildHeaders({ accessToken: "sk-ant-oat-x", context1m: false }))).not.toContain("context-1m-2025-08-07");
  });
});
