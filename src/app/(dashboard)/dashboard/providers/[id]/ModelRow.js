import PropTypes from "prop-types";
import { CapacityBadges } from "@/shared/components";

// Compact token formatter: 1000000 → "1M", 200000 → "200K".
function fmtTokens(n) {
  if (!n || typeof n !== "number") return "";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n % 1_000_000 === 0 ? 0 : 1)}M`;
  if (n >= 1_000) return `${Math.round(n / 1_000)}K`;
  return String(n);
}

export default function ModelRow({ model, fullModel, alias, copied, onCopy, testStatus, isCustom, isFree, onDeleteAlias, onTest, isTesting, onDisable, caps, effort, onEffortChange, context, onContextChange, showContext }) {
  const borderColor = testStatus === "ok"
    ? "border-green-500/40"
    : testStatus === "error"
    ? "border-red-500/40"
    : "border-border";

  const iconColor = testStatus === "ok"
    ? "#22c55e"
    : testStatus === "error"
    ? "#ef4444"
    : undefined;

  // Effort selector: only for reasoning models; offer xhigh/max only when the model supports them.
  const showEffort = !!caps?.reasoning && typeof onEffortChange === "function";
  const ceil = caps?.maxEffort || "high";
  const effortOpts = ["auto", "none", "low", "medium", "high",
    ...(ceil === "xhigh" || ceil === "max" ? ["xhigh"] : []),
    ...(ceil === "max" ? ["max"] : [])];
  const cap = (s) => s.charAt(0).toUpperCase() + s.slice(1);

  return (
    <div className={`group min-w-0 max-w-full rounded-lg border px-3 py-2 ${borderColor} hover:bg-sidebar/50`}>
      <div className="flex min-w-0 items-start gap-2 sm:items-center">
        <span
          className="material-symbols-outlined shrink-0 text-base"
          style={iconColor ? { color: iconColor } : undefined}
        >
          {testStatus === "ok" ? "check_circle" : testStatus === "error" ? "cancel" : "smart_toy"}
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <code className="max-w-[72vw] truncate rounded bg-sidebar px-1.5 py-0.5 font-mono text-xs text-text-muted sm:max-w-[360px]">{fullModel}</code>
          <span className="flex min-w-0 flex-wrap items-center text-[9px] gap-1 pl-1">
            {model.name && <span className="truncate text-[9px] italic text-text-muted/70">{model.name}</span>}
            <CapacityBadges caps={caps} colorOverride="text-text-muted/70" size={12} />
            {caps?.contextWindow ? (
              <span
                className="rounded bg-sidebar px-1 text-[9px] font-medium text-text-muted/70"
                title={`Context window: ${caps.contextWindow.toLocaleString()} tokens${caps.maxOutput ? ` · max output ${caps.maxOutput.toLocaleString()}` : ""}`}
              >
                {fmtTokens(caps.contextWindow)} ctx
              </span>
            ) : null}
            {caps?.maxEffort && caps.maxEffort !== "high" ? (
              <span
                className="rounded bg-sidebar px-1 text-[9px] font-medium text-amber-500/80"
                title={`Supports reasoning effort up to "${caps.maxEffort}"`}
              >
                {caps.maxEffort} effort
              </span>
            ) : null}
          </span>
        </div>
        {onTest && (
          <div className="relative shrink-0 group/btn">
            <button
              onClick={onTest}
              disabled={isTesting}
              className={`rounded p-0.5 text-text-muted transition-opacity hover:bg-sidebar hover:text-primary ${isTesting ? "opacity-100" : "opacity-100 sm:opacity-0 sm:group-hover:opacity-100"}`}
            >
              <span className="material-symbols-outlined text-sm" style={isTesting ? { animation: "spin 1s linear infinite" } : undefined}>
                {isTesting ? "progress_activity" : "science"}
              </span>
            </button>
            <span className="pointer-events-none absolute mt-1 top-5 left-1/2 -translate-x-1/2 text-[10px] text-text-muted whitespace-nowrap opacity-0 group-hover/btn:opacity-100 transition-opacity">
              {isTesting ? "Testing..." : "Test"}
            </span>
          </div>
        )}
        <div className="relative shrink-0 group/btn">
          <button
            onClick={() => onCopy(fullModel, `model-${model.id}`)}
            className="rounded p-0.5 text-text-muted hover:bg-sidebar hover:text-primary"
          >
            <span className="material-symbols-outlined text-sm">
              {copied === `model-${model.id}` ? "check" : "content_copy"}
            </span>
          </button>
          <span className="pointer-events-none absolute mt-1 top-5 left-1/2 -translate-x-1/2 text-[10px] text-text-muted whitespace-nowrap opacity-0 group-hover/btn:opacity-100 transition-opacity">
            {copied === `model-${model.id}` ? "Copied!" : "Copy"}
          </span>
        </div>
        {isCustom ? (
          <button
            onClick={onDeleteAlias}
            className="ml-auto rounded p-0.5 text-text-muted opacity-100 transition-opacity hover:bg-red-500/10 hover:text-red-500 sm:opacity-0 sm:group-hover:opacity-100"
            title="Remove custom model"
          >
            <span className="material-symbols-outlined text-sm">close</span>
          </button>
        ) : onDisable ? (
          <button
            onClick={onDisable}
            className="ml-auto rounded p-0.5 text-text-muted opacity-100 transition-opacity hover:bg-red-500/10 hover:text-red-500 sm:opacity-0 sm:group-hover:opacity-100"
            title="Disable this model"
          >
            <span className="material-symbols-outlined text-sm">close</span>
          </button>
        ) : null}
      </div>
      {(showEffort || (showContext && typeof onContextChange === "function")) && (
        <div className="mt-1.5 flex flex-wrap items-center gap-1.5 pl-7">
          {showEffort && (
            <select
              value={effort || "auto"}
              onChange={(e) => onEffortChange(e.target.value)}
              title="Reasoning effort for this model"
              className="rounded border border-border bg-background px-1 py-0.5 text-[10px] text-text-muted focus:border-primary focus:outline-none"
            >
              {effortOpts.map((o) => (
                <option key={o} value={o}>{o === "auto" ? "Effort: Auto" : `Effort: ${cap(o)}`}</option>
              ))}
            </select>
          )}
          {showContext && typeof onContextChange === "function" && (
            <select
              value={context || "auto"}
              onChange={(e) => onContextChange(e.target.value)}
              title="Context window — 1M sends Anthropic's context-1m beta header"
              className="rounded border border-border bg-background px-1 py-0.5 text-[10px] text-text-muted focus:border-primary focus:outline-none"
            >
              <option value="auto">Ctx: Auto</option>
              <option value="200k">Ctx: 200K</option>
              <option value="1m">Ctx: 1M</option>
            </select>
          )}
        </div>
      )}
    </div>
  );
}

ModelRow.propTypes = {
  model: PropTypes.shape({
    id: PropTypes.string.isRequired,
  }).isRequired,
  fullModel: PropTypes.string.isRequired,
  alias: PropTypes.string,
  copied: PropTypes.string,
  onCopy: PropTypes.func.isRequired,
  testStatus: PropTypes.oneOf(["ok", "error"]),
  isCustom: PropTypes.bool,
  isFree: PropTypes.bool,
  onDeleteAlias: PropTypes.func,
  onTest: PropTypes.func,
  isTesting: PropTypes.bool,
  onDisable: PropTypes.func,
  caps: PropTypes.object,
  effort: PropTypes.string,
  onEffortChange: PropTypes.func,
  context: PropTypes.string,
  onContextChange: PropTypes.func,
  showContext: PropTypes.bool,
};
