#!/usr/bin/env node
import readline from 'node:readline';

const rl = readline.createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
});

const mode = process.argv.includes('--fail') ? 'fail' : 'success';

function send(payload) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', ...payload })}\n`);
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
          name: 'Fake ACP Agent',
          version: '0.1.0',
        },
        capabilities: {
          sessions: true,
        },
      },
    });
    return;
  }

  if (request.method === 'session/new') {
    send({
      id: request.id,
      result: {
        sessionId: `fake-session-${Date.now()}`,
        configOptions: [
          {
            id: 'fixture-small',
            label: 'Fixture Small',
            category: 'model',
            valueType: 'select',
            choices: ['fixture-small', 'fixture-large'],
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
          message: 'Fake ACP agent forced failure',
        },
      });
      return;
    }

    const prompt = request.params?.prompt
      ?.map((part) => part.text)
      .filter(Boolean)
      .join('\n') ?? '';
    send({
      id: request.id,
      result: {
        content: [
          {
            type: 'text',
            text: `Fake ACP Agent completed prompt: ${prompt.slice(0, 160)}`,
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

