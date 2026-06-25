// Regression tests for the Claude Opus 4.8 enhancement on the Claude Code (cc)
// and GitHub Copilot (gh) providers: 1M context window + xhigh/max effort.
import { describe, it, expect } from "vitest";
import { getCapabilitiesForModel } from "../../open-sse/providers/capabilities.js";
import { stripUnsupportedParams } from "../../open-sse/translator/concerns/paramSupport.js";

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
