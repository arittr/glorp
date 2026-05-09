#!/usr/bin/env node
console.log(JSON.stringify({
  daily: [{
    date: "2026-05-09",
    inputTokens: 100,
    outputTokens: 200,
    cacheCreationTokens: 50,
    cacheReadTokens: 10000,
    totalCost: 0.12,
    modelsUsed: ["claude-sonnet-4"],
    prompt: "secret prompt",
    response: "secret response",
    toolCall: { arguments: "secret tool payload" }
  }]
}));
