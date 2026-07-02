#!/usr/bin/env node
import readline from 'node:readline';

const rl = readline.createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
});

const mode = process.argv.includes('--fail') ? 'fail' : 'success';
const stream = process.argv.includes('--stream');

function send(payload) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', ...payload })}\n`);
}

function readPrompt(params) {
  const prompt = params?.prompt;
  if (!Array.isArray(prompt)) {
    return '';
  }
  return prompt
    .map((part) => part?.text)
    .filter(Boolean)
    .join('\n');
}

rl.on('line', (line) => {
  if (!line.trim()) {
    return;
  }

  let request;
  try {
    request = JSON.parse(line);
  } catch (error) {
    send({
      id: null,
      error: {
        code: -32700,
        message: `Parse error: ${error.message}`,
      },
    });
    return;
  }

  if (request.method === 'initialize') {
    send({
      id: request.id,
      result: {
        protocolVersion: 1,
        agentInfo: {
          name: '模拟 ACP 智能体',
          version: '0.1.0',
        },
        agentCapabilities: {
          promptCapabilities: {},
        },
      },
    });
    return;
  }

  if (request.method === 'session/new') {
    if (typeof request.params?.cwd !== 'string' || request.params.cwd.length === 0) {
      send({
        id: request.id,
        error: {
          code: -32602,
          message: 'Invalid params',
          data: {
            cwd: ['expected non-empty string'],
          },
        },
      });
      return;
    }

    send({
      id: request.id,
      result: {
        sessionId: `fake-session-${Date.now()}`,
        configOptions: [
          {
            id: 'fixture-small',
            name: 'Fixture Small',
            category: 'model',
            type: 'select',
            currentValue: 'fixture-small',
            options: [
              { value: 'fixture-small', name: 'Fixture Small' },
              { value: 'fixture-large', name: 'Fixture Large' },
            ],
          },
        ],
      },
    });
    return;
  }

  if (request.method === 'session/prompt') {
    if (mode === 'fail') {
      send({
        id: request.id,
        error: {
          code: -32000,
          message: '模拟 ACP 智能体按要求返回失败',
        },
      });
      return;
    }

    const text = `模拟 ACP 智能体已完成提示词：${readPrompt(request.params).slice(0, 160)}`;
    if (stream) {
      send({
        method: 'session/update',
        params: {
          sessionId: request.params?.sessionId,
          update: {
            sessionUpdate: 'agent_message_chunk',
            messageId: 'fake-message',
            content: {
              type: 'text',
              text,
            },
          },
        },
      });
      send({
        id: request.id,
        result: {
          stopReason: 'end_turn',
        },
      });
      return;
    }

    send({
      id: request.id,
      result: {
        content: [
          {
            type: 'text',
            text,
          },
        ],
      },
    });
    return;
  }

  send({
    id: request.id,
    error: {
      code: -32601,
      message: `Unknown method ${request.method}`,
    },
  });
});
