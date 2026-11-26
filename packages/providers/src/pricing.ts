/**
 * Provider cost estimates in USD per million tokens.
 *
 * These figures feed only the engine's budget guard — they are estimates, not
 * billing data. Provider pricing changes over time; review and bump
 * PRICING_VERSION when updating. Do not treat engine cost output as exact.
 */
export const PRICING_VERSION = '2025-01';

export interface TokenPrice {
  readonly inputPerMillion: number;
  readonly outputPerMillion: number;
}

export const PROVIDER_PRICING = {
  /** Anthropic Claude Sonnet-class list pricing. */
  anthropic: { inputPerMillion: 3, outputPerMillion: 15 },
  /** OpenAI GPT-4o-class list pricing, also applied to compatible endpoints. */
  openai: { inputPerMillion: 2.5, outputPerMillion: 10 },
  /**
   * ChatGPT subscription calls are not metered per token; this flat blended
   * estimate exists only so the budget guard has a cost signal.
   */
  codexSubscriptionBlended: { inputPerMillion: 5, outputPerMillion: 5 },
} as const satisfies Record<string, TokenPrice>;

export function estimateUsd(price: TokenPrice, inputTokens: number, outputTokens: number): number {
  return (inputTokens * price.inputPerMillion + outputTokens * price.outputPerMillion) / 1_000_000;
}
