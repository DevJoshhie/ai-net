# Venice AI Prompting Guide

This guide is for developers building research, risk, coding, design, and report agents on ai-net. It covers model selection, prompt structure, context and cost budgets, batch and streaming requests, and failure handling.

## API and Model Selection

Venice provides an OpenAI-compatible chat completions endpoint:

```text
POST https://api.venice.ai/api/v1/chat/completions
Authorization: Bearer $VENICE_API_KEY
```

Use the model catalog to confirm IDs, availability, context limits, capabilities, and current rates before deploying. The table below reflects the public catalog queried on **2026-10-22**. Prices are USD per 1 million input/output tokens; the catalog is authoritative and rates can change.

| Model ID | Suggested use | Input / 1M tokens | Output / 1M tokens | Context tokens |
|---|---|---:|---:|---:|
| `qwen-3-8-flash` | Research triage, design briefs, report drafts | $0.14 | $0.49 | 1,000,000 |
| `z-ai-glm-5-3-flash` | Lower-cost risk analysis and structured extraction | $0.15 | $0.50 | 1,048,576 |
| `qwen3-coder-480b-a35b-instruct-turbo` | Code generation and review | $0.35 | $1.50 | 256,000 |
| `llama-3.3-70b` | General-purpose analysis and synthesis | $0.70 | $2.80 | 128,000 |
| `z-ai-glm-5-3` | Complex, high-consequence risk analysis | $1.75 | $5.50 | 1,000,000 |

These are starting points, not quality guarantees. Evaluate candidates with representative tasks, a fixed rubric, and a token budget before selecting a production model. ai-net's backend currently maps agent kinds to `venice-xl` and `venice-code`; those IDs were not present in the public catalog when this guide was checked. Verify or update that mapping against the live catalog before relying on it. The examples below use catalog IDs.

## Prompt Patterns

Keep system instructions stable and put task-specific data in the user message. State the audience, scope, evidence rules, output shape, and what the model should do when information is missing. Treat output as untrusted input: validate structured responses before using them.

The following templates cover the five ai-net agent types:

```typescript
type AgentKind = 'research' | 'risk' | 'coding' | 'design' | 'report';

const systemPrompts: Record<AgentKind, string> = {
  research: `You are a research analyst. Separate sourced facts from inference.
Return a concise summary, key findings, and sources with title and URL.
Do not invent citations; state when evidence is unavailable.`,
  risk: `You are a risk analyst. Identify concrete risks, their evidence,
likelihood and impact, mitigations, and unresolved assumptions. Do not present
an estimate as a verified fact. Return a structured, concise analysis.`,
  coding: `You are a senior software engineer. Use only the supplied repository
context and requirements. Propose the smallest correct change, explain edge
cases, and include focused verification steps. Never claim a command or test
was run unless it actually was.`,
  design: `You are a product designer. Respect the supplied users, platform,
accessibility requirements, and design constraints. Return a clear hierarchy,
interaction states, responsive behavior, and rationale; flag conflicts rather
than silently ignoring requirements.`,
  report: `You are a report editor. Synthesize only the supplied findings,
preserve attribution and uncertainty, remove repetition, and distinguish
conclusions from open questions. Return an executive summary followed by
evidence-based findings and sources.`,
};
```

For example, ask the research agent for a market comparison with a bounded date range and explicit source requirements. Ask the risk agent to assess a named threat model, not to declare a system “safe.” Give the coding agent the relevant files and acceptance criteria rather than an entire repository dump. Give the design agent viewport, accessibility, and component constraints. Ask the report agent to synthesize validated upstream results and retain their source IDs.

## TypeScript: Batch Requests

This minimal helper uses the native `fetch` API and the documented chat-completions response shape. Keep `VENICE_API_KEY` in the process environment or a secret manager; never put it in source control or client-side code.

```typescript
type Message = { role: 'system' | 'user' | 'assistant'; content: string };
type ChatResponse = { choices?: Array<{ message?: { content?: string } }> };

async function complete(
  model: string,
  messages: Message[],
  maxTokens = 800,
): Promise<string> {
  const apiKey = process.env.VENICE_API_KEY;
  if (!apiKey) throw new Error('VENICE_API_KEY is required');

  const response = await fetch('https://api.venice.ai/api/v1/chat/completions', {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${apiKey}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ model, messages, max_tokens: maxTokens, temperature: 0.2 }),
    signal: AbortSignal.timeout(15_000),
  });
  if (!response.ok) {
    throw new Error(`Venice returned HTTP ${response.status}: ${await response.text()}`);
  }

  const data = (await response.json()) as ChatResponse;
  const content = data.choices?.[0]?.message?.content;
  if (typeof content !== 'string') throw new Error('Venice response has no message content');
  return content;
}

const result = await complete('qwen-3-8-flash', [
  { role: 'system', content: systemPrompts.research },
  { role: 'user', content: 'Compare the supplied market evidence for 2025. Cite only the supplied sources.' },
]);
console.log(result);
```

Use `max_tokens` to cap generated output. ai-net's backend defaults to 2,048 completion tokens and rejects requests above its 8,192-token hard cap; the task API also limits prompt characters. These are ai-net safeguards, not Venice-wide limits.

## Context and Token Budgets

- Reserve room for the system prompt, user request, retrieved context, and expected output within the selected model's context window.
- Tokenization varies by model. `Math.ceil(text.length / 4)` is only a rough early warning; use a tokenizer appropriate to the selected model for production budgets.
- For long inputs, split at semantic boundaries (documents, sections, or records), not arbitrary character offsets. Keep source IDs and metadata attached to every chunk.
- Summarize chunks into a fixed schema with source IDs, then synthesize those summaries. Retain access to the original passages for citation checks; summaries can omit important qualifiers.
- Limit retrieval to the most relevant, non-duplicated passages. Do not repeatedly resend the full conversation when a compact state summary will do.
- Set maximum input and output budgets per task type and stop or request clarification when the budget is insufficient.

### Estimating Cost

Use the provider's returned usage values where available. Before a request, estimate with:

```text
estimated USD = input tokens * input USD per 1M / 1,000,000
              + output tokens * output USD per 1M / 1,000,000
```

Example using `qwen-3-8-flash`: 12,000 input tokens and 1,500 output tokens at the listed rates estimate to `$0.002415` before any provider-specific adjustments. Retries and uncached calls add cost; caching can reduce repeated identical work, but do not assume a cache hit. Use measured usage and current rates for billing decisions.

## Streaming Responses

Streaming sends `stream: true` and returns server-sent event frames. The example below demonstrates the contract; production code should also handle cancellation, partial frames, and request timeouts.

```typescript
type StreamPayload = { choices?: Array<{ delta?: { content?: string } }> };

async function streamCompletion(
  model: string,
  messages: Message[],
  onToken: (token: string) => void,
): Promise<void> {
  const apiKey = process.env.VENICE_API_KEY;
  if (!apiKey) throw new Error('VENICE_API_KEY is required');

  const response = await fetch('https://api.venice.ai/api/v1/chat/completions', {
    method: 'POST',
    headers: { Authorization: `Bearer ${apiKey}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({ model, messages, max_tokens: 800, stream: true }),
  });
  if (!response.ok) throw new Error(`Venice returned HTTP ${response.status}`);
  if (!response.body) throw new Error('Venice response has no stream body');

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  for (;;) {
    const { done, value } = await reader.read();
    if (value) buffer += decoder.decode(value, { stream: !done });
    if (done) buffer += decoder.decode();

    let boundary = buffer.indexOf('\n\n');
    while (boundary !== -1) {
      const frame = buffer.slice(0, boundary);
      buffer = buffer.slice(boundary + 2);
      for (const line of frame.split('\n')) {
        if (!line.startsWith('data: ') || line === 'data: [DONE]') continue;
        const payload = JSON.parse(line.slice(6)) as StreamPayload;
        const token = payload.choices?.[0]?.delta?.content;
        if (typeof token === 'string') onToken(token);
      }
      boundary = buffer.indexOf('\n\n');
    }
    if (done) break;
  }
}
```

Use streaming when partial text improves perceived latency, such as a user-facing draft. Use a batch response when the caller needs a complete result before validation, JSON parsing, or an on-chain action. Do not treat partial streamed output as a completed or validated result.

## Errors and Local Development

| Condition | Recommended handling |
|---|---|
| `401` or `403` | Stop retries; check key validity, permissions, and secret configuration. |
| `400` or `422` | Inspect the model ID and request schema; correct the request instead of retrying unchanged input. |
| `429` | Honor `Retry-After` when provided; otherwise use bounded exponential backoff with jitter and enforce a per-task retry budget. |
| `5xx`, timeout, or network failure | Retry a small bounded number of times, then fail gracefully or use a permitted cached result. Avoid retry storms. |
| Model unavailable | Check the live model catalog and select an explicitly supported alternative; do not silently change models for safety-sensitive tasks. |
| Refusal or filtered output | Surface a safe, actionable error. Do not retry by weakening policy or disguising the request. |
| Invalid structured output | Parse and validate against a schema. Allow at most a bounded repair attempt, and never trust unvalidated fields. |

For local development, set a development-only key through an ignored `.env` file, keep `max_tokens` small, and send a short representative prompt. Mock the HTTP boundary for deterministic unit checks; do not make CI depend on a paid live API. Record model ID, latency, status, retry count, and provider usage, but never log API keys or full sensitive prompts. ai-net's client already uses timeouts, bounded retries, provider failover, a circuit breaker, and optional cached fallback; direct API callers should implement equivalent controls appropriate to their application.

## Full System Prompt Templates

The templates in [Prompt Patterns](#prompt-patterns) show the minimum viable content. Production agents use an extended structure that injects versioned context, enforces an output schema, and requests a confidence score. The pattern below applies to all five agent types; adapt the role declaration and output schema for each.

```typescript
// prompts/v1.0/research.ts
// NEVER commit real wallet addresses, API keys, or user data into prompt files.
// Sensitive runtime values (walletAddress, sessionId) are injected at call time.

export const PROMPT_VERSION = '1.0';

export interface PromptMetadata {
  version: string;
  agentKind: AgentKind;
  createdAt: string;   // ISO 8601 date this version was authored
  changedFields: string[]; // fields altered from the previous version
}

export const metadata: PromptMetadata = {
  version: PROMPT_VERSION,
  agentKind: 'research',
  createdAt: '2025-01-01',
  changedFields: [],
};

/**
 * Build a complete system prompt for the research agent.
 *
 * @param context  - Retrieved passages, each carrying a sourceId. Injected at
 *                   call time, not stored in this file.
 * @param outputSchema - JSON Schema string for the expected response shape.
 */
export function buildResearchSystemPrompt(
  context: { sourceId: string; text: string }[],
  outputSchema: string,
): string {
  // 1. Role declaration
  const role = `You are a research analyst working for ai-net. \
Your sole purpose is to synthesize information from the supplied context \
into concise, evidence-based findings.`;

  // 2. Context injection pattern — serialize retrieved passages with IDs so
  //    the model can cite them without inventing references.
  const contextBlock = context.length
    ? context.map(c => `[${c.sourceId}] ${c.text}`).join('\n\n')
    : 'No external context was supplied for this request.';

  // 3. Output format instruction
  const outputInstruction = `Return your response as a single JSON object \
that conforms exactly to this schema:\n${outputSchema}\n\
Do not include any text outside the JSON object.`;

  // 4. Confidence scoring instruction
  const confidenceInstruction = `Include a "confidence" field (number 0.0–1.0) \
in your response. Derive the score as follows:
  - Start at 1.0.
  - Subtract 0.1 for each key claim that lacks a citation in the supplied context.
  - Subtract 0.2 if the context is older than 90 days or its recency is unknown.
  - Subtract 0.1 for each significant assumption you had to make.
  - Floor at 0.0.
State the deductions in a "confidenceRationale" field (string).`;

  return [role, `\n## Supplied Context\n${contextBlock}`, outputInstruction, confidenceInstruction].join('\n\n');
}

// ─── Risk agent ───────────────────────────────────────────────────────────────
// prompts/v1.0/risk.ts (abbreviated — same four-part structure)
//
// Role: "You are a risk analyst. Identify concrete risks with evidence,
//        likelihood, impact, and mitigations. Do not present an estimate as a
//        verified fact."
// Context injection: threat-model document and on-chain data passages.
// Output format: JSON schema with fields: risks[], overallRiskLevel, confidence,
//                confidenceRationale.
// Confidence instruction: (same formula as above)

// ─── Coding agent ─────────────────────────────────────────────────────────────
// prompts/v1.0/coding.ts
//
// Role: "You are a senior software engineer. Use only the supplied repository
//        context and requirements. Propose the smallest correct change."
// Context injection: relevant source files (path + content), acceptance criteria.
// Output format: JSON with fields: explanation, fileDiffs[], verificationSteps[],
//                confidence, confidenceRationale.
// Confidence instruction: deduct for untested assumptions about runtime behavior.

// ─── Design agent ─────────────────────────────────────────────────────────────
// prompts/v1.0/design.ts
//
// Role: "You are a product designer. Respect supplied users, platform,
//        accessibility requirements, and design constraints."
// Context injection: component inventory, viewport constraints, WCAG level.
// Output format: JSON with fields: hierarchy, interactionStates[], responsive,
//                rationale, conflicts[], confidence, confidenceRationale.

// ─── Report agent ─────────────────────────────────────────────────────────────
// prompts/v1.0/report.ts
//
// Role: "You are a report editor. Synthesize only the supplied findings.
//        Preserve attribution and uncertainty."
// Context injection: upstream agent outputs (each carrying an agentId and
//                   confidence score from the originating agent).
// Output format: JSON with fields: executiveSummary, findings[], sources[],
//                openQuestions[], confidence, confidenceRationale.
// Confidence instruction: weight by the confidence scores of the upstream agents.
```

---

## JSON Output Mode

Venice's chat-completions endpoint accepts a `response_format` parameter. Setting it to `{ "type": "json_object" }` instructs the model to produce a valid JSON object as its entire response. Use this for all structured agent outputs so callers can parse without regex or heuristics.

### Request structure

```typescript
import Ajv, { JSONSchemaType } from 'ajv';

interface ResearchOutput {
  summary: string;
  findings: Array<{ claim: string; sourceId: string }>;
  confidence: number;       // 0.0 – 1.0
  confidenceRationale: string;
}

const researchSchema: JSONSchemaType<ResearchOutput> = {
  type: 'object',
  properties: {
    summary:              { type: 'string' },
    findings:             {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          claim:    { type: 'string' },
          sourceId: { type: 'string' },
        },
        required: ['claim', 'sourceId'],
        additionalProperties: false,
      },
    },
    confidence:           { type: 'number', minimum: 0, maximum: 1 },
    confidenceRationale:  { type: 'string' },
  },
  required: ['summary', 'findings', 'confidence', 'confidenceRationale'],
  additionalProperties: false,
};

const ajv = new Ajv();
const validateResearch = ajv.compile(researchSchema);

async function completeStructured<T>(
  model: string,
  messages: Message[],
  validate: (data: unknown) => data is T,
  maxRetries = 3,
): Promise<T> {
  const apiKey = process.env.VENICE_API_KEY;
  if (!apiKey) throw new Error('VENICE_API_KEY is required');

  let lastRaw = '';
  for (let attempt = 0; attempt < maxRetries; attempt++) {
    // On retry attempts, append a correction message rather than resending the
    // whole conversation — it keeps the token cost bounded and gives the model
    // specific feedback.
    const retryMessages: Message[] =
      attempt === 0
        ? messages
        : [
            ...messages,
            { role: 'assistant', content: lastRaw },
            {
              role: 'user',
              content:
                'Your previous response was not valid JSON or did not match ' +
                'the required schema. Return only a valid JSON object that ' +
                'matches the schema. Do not include any other text.',
            },
          ];

    const response = await fetch('https://api.venice.ai/api/v1/chat/completions', {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${apiKey}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        model,
        messages: retryMessages,
        max_tokens: 2048,
        temperature: 0.2,
        response_format: { type: 'json_object' }, // enforce JSON output
      }),
      signal: AbortSignal.timeout(20_000),
    });

    if (!response.ok) {
      throw new Error(`Venice returned HTTP ${response.status}: ${await response.text()}`);
    }

    const data = (await response.json()) as ChatResponse;
    lastRaw = data.choices?.[0]?.message?.content ?? '';

    let parsed: unknown;
    try {
      parsed = JSON.parse(lastRaw);
    } catch {
      // Malformed JSON — retry with the correction prompt above.
      continue;
    }

    if (validate(parsed)) return parsed;
    // Parsed but schema-invalid — retry with the correction prompt.
  }

  throw new Error(
    `Venice did not return a valid structured response after ${maxRetries} attempts. ` +
    `Last raw output: ${lastRaw.slice(0, 300)}`,
  );
}

// Usage
const result = await completeStructured(
  'qwen-3-8-flash',
  [
    { role: 'system', content: buildResearchSystemPrompt(retrievedContext, JSON.stringify(researchSchema)) },
    { role: 'user', content: 'Analyze the supplied market data for Q3 2025.' },
  ],
  validateResearch as (d: unknown) => d is ResearchOutput,
);
```

Key points:
- `response_format: { type: "json_object" }` is set on every structured request.
- The system prompt also contains an explicit output schema (see [Full System Prompt Templates](#full-system-prompt-templates)), giving the model two independent signals.
- The retry loop sends up to three attempts. On retries it appends a correction user message instead of resubmitting the full prompt, which bounds token cost and gives specific guidance.
- After three failures the function throws; the caller decides whether to surface a degraded response or fail the task.

---

## Confidence Scoring System

### Formula

Each agent computes its confidence score from 1.0 by subtracting penalties:

```
confidence = max(0.0,
  1.0
  − 0.1 × (unsupported_claims)
  − 0.2 × (context_stale_or_unknown_recency)
  − 0.1 × (significant_assumptions)
)
```

Where:
- **unsupported_claims** — count of key claims with no matching `[sourceId]` citation in the supplied context.
- **context_stale_or_unknown_recency** — 1 if any supplied context chunk is older than 90 days or carries no timestamp, 0 otherwise.
- **significant_assumptions** — count of assumptions the model had to make beyond what the context provides.

The formula is stated verbatim in each agent's system prompt (see the `confidenceInstruction` block in [Full System Prompt Templates](#full-system-prompt-templates)), so the model calculates and reports it rather than the backend inferring it post-hoc.

### Embedding in the response

Every structured agent output includes two fields:

```json
{
  "summary": "...",
  "findings": [
    { "claim": "Token volume grew 34% QoQ.", "sourceId": "src-001" },
    { "claim": "Competitor X launched on mainnet.", "sourceId": "src-002" }
  ],
  "confidence": 0.7,
  "confidenceRationale": "Deducted 0.2 for stale context (sources dated > 90 days ago). Deducted 0.1 for one claim with no direct citation."
}
```

### Downstream behavior

| Confidence range | Action |
|---|---|
| ≥ 0.7 | Result accepted and surfaced normally. |
| 0.5 – < 0.7 | Result flagged for human review before any on-chain or user-facing action. Reputation impact for the agent is reduced by 50%. |
| < 0.5 | Result quarantined. Task is re-queued with a richer context payload or escalated. No reputation impact is applied. |

The backend reads `confidence` after schema validation and routes accordingly. Report agents further weight their own output confidence by the confidence scores of the upstream research/risk agents they synthesize.

---

## Prompt Versioning Strategy

### File naming convention

```
backend/src/prompts/v{major}.{minor}/{agent_type}.ts
```

Examples:
```
backend/src/prompts/v1.0/research.ts
backend/src/prompts/v1.0/risk.ts
backend/src/prompts/v1.1/research.ts   ← minor bump after wording refinement
backend/src/prompts/v2.0/research.ts   ← major bump after output schema change
```

Each file exports a `metadata` object (see [Full System Prompt Templates](#full-system-prompt-templates)) and the builder function. The active version per agent type is set in `backend/src/prompts/index.ts`, which re-exports the builders callers import:

```typescript
// backend/src/prompts/index.ts
export { buildResearchSystemPrompt, metadata as researchMeta } from './v1.1/research';
export { buildRiskSystemPrompt,     metadata as riskMeta     } from './v1.0/risk';
// ... other agents
```

### When to bump

| Change | Version impact |
|---|---|
| Wording tweak with no behavioral effect | No bump — amend in place and note in `changedFields`. |
| Any change to output field names, types, or the confidence formula | Minor bump (e.g. `1.0` → `1.1`). |
| Adding or removing top-level output fields, changing the agent's role scope | Major bump (e.g. `1.1` → `2.0`). |

### A/B testing

To compare two versions in production, run both in parallel for a sample of tasks and compare on:
- Schema validation pass rate
- Mean confidence score
- Downstream human-review flag rate
- Task completion rate

```typescript
function selectPromptVersion(agentKind: AgentKind, trafficFraction: number): string {
  // trafficFraction ∈ [0, 1]: proportion of requests routed to the candidate version.
  return Math.random() < trafficFraction ? 'candidate' : 'stable';
}
```

Log the version identifier alongside every task result so metrics can be sliced by version. Promote the candidate if it improves the target metric by a statistically significant margin over at least 200 task completions per variant.

### Rollback

1. Revert the import in `backend/src/prompts/index.ts` to the previous version file.
2. Bump the minor version of the reverted file (e.g. `v1.1/research.ts` → `v1.2/research.ts`) so the deployment history remains monotonically increasing and the rollback is recorded.
3. Add a `changedFields` entry in the new file's metadata: `"reverted to v1.0 behavior due to elevated review flag rate"`.
4. Deploy. No database migration is required because the version identifier is logged per task, not stored as a foreign key.

---

## Hallucination Mitigation

Apply these techniques through explicit prompt instructions. Each instruction below is used verbatim (or parameterized as shown) in ai-net's agent system prompts.

### 1. Chain-of-thought forcing

> "Before answering, reason step-by-step in a `<reasoning>` block, then provide your final answer."

Place this in the system prompt. The model's visible reasoning trace makes unsupported leaps detectable during review and tends to reduce the rate at which the final answer contradicts the context.

### 2. Explicit uncertainty acknowledgment

> "If you are not certain about a fact, state your confidence level explicitly using: 'Confidence: X%'."

This applies inside `<reasoning>` blocks. The numeric claim is extracted and cross-checked against the top-level `confidence` field; a large divergence triggers a review flag.

### 3. Source citation requirement (research and report agents)

> "Every factual claim must be followed by a [Source: URL or document name] citation. Do not make claims without citations."

For agents that receive structured context with `sourceId` fields, this instruction is sharpened to: "Every factual claim must cite exactly one `[sourceId]` from the supplied context. Do not invent source IDs."

### 4. Refusal instruction

> "If the request is outside your designated scope, respond with: `{'error': 'out_of_scope', 'message': '<explanation>'}`. Do not attempt to answer out-of-scope requests."

The scope boundary is defined per agent type. For example, the coding agent's scope is "changes to the supplied repository files"; requests to summarize news or perform financial analysis are out of scope. The structured error shape is validated like any other response, so out-of-scope refusals are caught and surfaced rather than silently dropped.

### 5. Grounding instruction

> "Base your response only on the context provided. Do not introduce facts or data not present in the supplied context."

This is the first instruction in every system prompt that injects context. It is stated before the context block so the model encounters it before reading the supplied passages.

### 6. Output schema anchoring

> "Your response MUST conform exactly to this JSON schema: `{schema}`. Any deviation will cause a validation error."

The literal schema string is substituted for `{schema}` at call time (see `buildResearchSystemPrompt`). Stating the schema as a constraint rather than a description increases the rate of schema-valid first responses and reduces retry cost.

---

## Few-Shot Examples

Few-shot examples prime the model to match the exact output style and field population expected by downstream consumers. They are more effective than prose descriptions for tasks with a constrained output shape.

### Format

Embed examples as alternating `user`/`assistant` messages before the live user message:

```typescript
function buildMessagesWithExamples(
  systemPrompt: string,
  examples: Array<{ userMessage: string; assistantResponse: string }>,
  liveUserMessage: string,
): Message[] {
  const exampleMessages: Message[] = examples.flatMap(ex => [
    { role: 'user' as const,      content: ex.userMessage },
    { role: 'assistant' as const, content: ex.assistantResponse },
  ]);

  return [
    { role: 'system', content: systemPrompt },
    ...exampleMessages,
    { role: 'user', content: liveUserMessage },
  ];
}
```

### How many examples per capability

Include 2–3 examples per distinct capability a prompt covers. More than 3 examples per capability rarely improves output quality and increases token cost on every request. Prioritize:
1. The most common success case.
2. A boundary or edge case (empty context, partial data, low confidence result).
3. An out-of-scope refusal, so the model sees the correct refusal shape.

### Example update process

1. When output quality degrades on a known case, add or replace the closest example.
2. Run the prompt test harness (see [Testing Prompt Changes](#testing-prompt-changes)) over the full example set before merging.
3. If adding an example changes the output on previously passing test cases, treat that as a breaking change and bump the prompt version accordingly.
4. Store examples alongside their prompt version file: `backend/src/prompts/v1.1/research.examples.ts`.

---

## Token Budget Management

### Context window sizes per model

| Model ID | Context window (tokens) | Suggested input budget | Suggested output cap |
|---|---:|---:|---:|
| `qwen-3-8-flash` | 1,000,000 | 950,000 | 4,096 |
| `z-ai-glm-5-3-flash` | 1,048,576 | 995,000 | 4,096 |
| `qwen3-coder-480b-a35b-instruct-turbo` | 256,000 | 240,000 | 8,192 |
| `llama-3.3-70b` | 128,000 | 120,000 | 4,096 |
| `z-ai-glm-5-3` | 1,000,000 | 950,000 | 8,192 |

The suggested input budget leaves a margin for the model's internal bookkeeping and avoids degraded performance near the absolute limit. The ai-net backend enforces a hard cap of 8,192 completion tokens regardless of model.

### Estimated tokens per prompt component

| Component | Typical token range | Notes |
|---|---|---|
| System prompt (role + instructions) | 200–500 | Stable; tokenize once at startup and cache the count. |
| Few-shot examples (2–3) | 300–900 | Depends on output schema complexity. |
| Retrieved context chunks | Variable (50–2,000 per chunk) | Budget the largest realistic payload at task design time. |
| Live user message | 20–200 | Usually short for structured agent tasks. |
| Expected output | Up to `max_tokens` | Set `max_tokens` to the minimum that covers the schema. |

Rule of thumb: `Math.ceil(text.length / 4)` over-estimates by 10–30% for English prose and can under-estimate for code or JSON. Use a tokenizer matched to the model family for precise budgets.

### Compression strategies

When context exceeds the input budget, apply these strategies in order:

```typescript
interface ContextChunk {
  sourceId: string;
  text: string;
  relevanceScore: number;  // 0.0–1.0, set by the retrieval layer
  tokenCount: number;
}

/**
 * Select the highest-relevance chunks that fit within the token budget.
 * Always preserves sourceId so citations remain valid.
 */
function selectChunksForBudget(
  chunks: ContextChunk[],
  budgetTokens: number,
): ContextChunk[] {
  // Sort descending by relevance so the most useful chunks are taken first.
  const sorted = [...chunks].sort((a, b) => b.relevanceScore - a.relevanceScore);

  let remaining = budgetTokens;
  const selected: ContextChunk[] = [];

  for (const chunk of sorted) {
    if (chunk.tokenCount <= remaining) {
      selected.push(chunk);
      remaining -= chunk.tokenCount;
    }
    // Skip chunks that don't fit rather than truncating mid-sentence.
  }

  return selected;
}

/**
 * Truncate a single chunk to a token ceiling while keeping whole sentences.
 * Use only when the highest-relevance chunk alone exceeds the budget.
 */
function truncateChunk(chunk: ContextChunk, maxTokens: number): ContextChunk {
  // Rough character ceiling; replace with a proper tokenizer in production.
  const maxChars = maxTokens * 4;
  if (chunk.text.length <= maxChars) return chunk;

  // Break at the last sentence boundary before the ceiling.
  const truncated = chunk.text.slice(0, maxChars);
  const lastPeriod = truncated.lastIndexOf('.');
  return {
    ...chunk,
    text: (lastPeriod > maxChars * 0.5 ? truncated.slice(0, lastPeriod + 1) : truncated) +
          ' [truncated]',
  };
}
```

Additional strategies:
- **De-duplicate**: remove chunks whose content overlaps > 80% with a higher-scored chunk.
- **Summarize stale context**: chunks older than 90 days can be pre-summarized into a fixed 100-token synopsis, preserving `sourceId`. Cache summaries; they don't change.
- **Compact conversation state**: do not resend the full message history on follow-up calls. Maintain a compact state summary (agent output fields, source IDs) and reconstruct only what the next call needs.

---

## Testing Prompt Changes

### Test harness

The backend ships a prompt test harness at `backend/src/prompts/__tests__/harness.ts`. It runs a prompt against a fixed set of recorded inputs and compares outputs against expected schemas and reference outputs.

```bash
# Run the full prompt test suite (uses recorded fixtures, no live API calls)
cd backend && npm run test -- --testPathPattern=prompts

# Run a single agent's prompt tests
cd backend && npm run test -- --testPathPattern=prompts/research
```

For evaluations that require live API calls (e.g. when updating few-shot examples or changing the model), use the evaluation script which routes to a development Venice key and records results:

```bash
# Requires VENICE_API_KEY in environment and writes results to .prompt-eval/
cd backend && npm run prompts:eval -- --agent=research --version=v1.1 --samples=50
```

### Metrics to compare

| Metric | Description | Collection method |
|---|---|---|
| Schema validation pass rate | Fraction of responses that parse and validate without retry | Logged per task in `task_results` table |
| First-attempt pass rate | Schema-valid on the first attempt (no retry) | Filter `attempt_number = 1` |
| Mean confidence score | Average `confidence` field across validated responses | Aggregated from `task_results` |
| Human-review flag rate | Fraction of responses routed to human review (confidence < 0.7) | `task_results.flagged_for_review` |
| Mean latency (ms) | End-to-end time from request to validated response | `task_results.duration_ms` |
| Estimated token cost (USD) | Using returned `usage` and current rate table | Computed from `task_results.input_tokens`, `output_tokens` |

### Minimum sample size

Use at least **200 task completions per variant** before drawing conclusions from A/B comparisons. For rare edge cases (out-of-scope refusals, low-confidence results), collect at least **50 occurrences of that event type** before comparing rates. Smaller samples produce high-variance estimates and can mask regressions.

For regression checks on a single prompt version (no A/B), run the fixed fixture suite (which covers known edge cases deterministically) plus a live sample of 30–50 representative tasks.

---

## References

- [Venice API model catalog](https://api.venice.ai/api/v1/models) — model IDs, context limits, capabilities, and price metadata.
- [Venice model-list API reference](https://docs.venice.ai/api-reference/endpoint/models/list)
- [Venice chat-completions API reference](https://docs.venice.ai/api-reference/endpoint/chat/completions)
- [ai-net Venice client](../backend/src/services/venice/client.ts) — internal agent mapping, retries, streaming, and token limits.
- [AI-Agent Integration Guide](ai-agent-integration-guide.md)