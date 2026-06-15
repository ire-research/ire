import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  ErrorCode,
  McpError,
} from '@modelcontextprotocol/sdk/types.js';
import net from 'net';
import { createInterface } from 'readline';

const SOCKET_PATH = process.env.IRE_BACKEND_SOCKET;

if (!SOCKET_PATH) {
  process.stderr.write('IRE_BACKEND_SOCKET not set\n');
  process.exit(1);
}

let requestId = 0;

function callBackend(method, params) {
  return new Promise((resolve, reject) => {
    const id = ++requestId;
    const client = net.createConnection(SOCKET_PATH);
    let settled = false;

    function settle(fn) {
      if (settled) return;
      settled = true;
      client.destroy();
      fn();
    }

    const rl = createInterface({ input: client });
    rl.once('line', (line) => {
      settle(() => {
        try {
          const resp = JSON.parse(line);
          if (resp.ok) {
            resolve(resp.result ?? {});
          } else {
            reject(new Error(resp.error ?? 'backend error'));
          }
        } catch (e) {
          reject(e);
        }
      });
    });

    client.once('connect', () => {
      client.write(JSON.stringify({ id, method, params }) + '\n');
    });

    client.once('error', (err) => settle(() => reject(err)));
    client.once('close', () => settle(() => reject(new Error('connection closed before response'))));
  });
}

const TOOLS = [
  {
    name: 'wiki.read',
    description: 'Read any wiki markdown file. Returns content and frontmatter.',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Path relative to wiki root (e.g. "notes.md", "pulse.json")' },
      },
      required: ['path'],
    },
  },
  {
    name: 'wiki.write',
    description: 'Atomically write a wiki file and update _index.md.',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Path relative to wiki root' },
        content: { type: 'string', description: 'File content' },
        summary: { type: 'string', description: 'One-line summary for the index (optional)' },
      },
      required: ['path', 'content'],
    },
  },
  {
    name: 'wiki.append',
    description: 'Append content to a wiki file.',
    inputSchema: {
      type: 'object',
      properties: {
        path: { type: 'string', description: 'Path relative to wiki root' },
        content: { type: 'string', description: 'Content to append' },
      },
      required: ['path', 'content'],
    },
  },
  {
    name: 'wiki.list',
    description: 'List all wiki file paths.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'wiki.rename',
    description: 'Atomically rename a wiki file and update _index.md.',
    inputSchema: {
      type: 'object',
      properties: {
        from: { type: 'string', description: 'Source path relative to wiki root' },
        to: { type: 'string', description: 'Destination path relative to wiki root' },
      },
      required: ['from', 'to'],
    },
  },
  {
    name: 'memory.write_long_term',
    description: 'Append an entry to long-term memory (long-term.md) under a named section. Use for architectural decisions, pivots, durable insights, and dead ends worth preserving.',
    inputSchema: {
      type: 'object',
      properties: {
        section: { type: 'string', description: 'Section heading (e.g. "Architecture Decision")' },
        content: { type: 'string', description: 'Content to append under the section' },
      },
      required: ['section', 'content'],
    },
  },
  {
    name: 'memory.write_short_term',
    description: "Append to today's short-term memory (short-term/YYYY-MM-DD.md). Use for daily operational notes, debugging steps, experiment details, and transient dead ends.",
    inputSchema: {
      type: 'object',
      properties: {
        content: { type: 'string', description: 'Content to append to today\'s notes' },
      },
      required: ['content'],
    },
  },
  {
    name: 'pulse.update',
    description: 'Update the research pulse (pulse.json). Patch either or both fields.',
    inputSchema: {
      type: 'object',
      properties: {
        research_question: { type: 'string', description: 'Current research question' },
        this_week: { type: 'string', description: 'Current weekly focus' },
      },
    },
  },
  {
    name: 'ask_user_question',
    description: 'Ask the user one or more multiple-choice questions and block until they respond in the IRE UI. Use this instead of guessing when you need the user to pick between options.',
    inputSchema: {
      type: 'object',
      properties: {
        questions: {
          type: 'array',
          description: 'Questions to ask, shown together as a form.',
          items: {
            type: 'object',
            properties: {
              header: { type: 'string', description: 'Short label for this question (e.g. "Approach")' },
              question: { type: 'string', description: 'The question text' },
              multiSelect: { type: 'boolean', description: 'Allow selecting multiple options (default false)' },
              options: {
                type: 'array',
                description: 'Choices the user can pick from',
                items: {
                  type: 'object',
                  properties: {
                    label: { type: 'string', description: 'Option label' },
                    description: { type: 'string', description: 'Optional explanation of this option' },
                  },
                  required: ['label'],
                },
                minItems: 1,
              },
            },
            required: ['header', 'question', 'options'],
          },
          minItems: 1,
        },
      },
      required: ['questions'],
    },
  },
  {
    name: 'experiment.start',
    description: 'Spawn a detached experiment subprocess. Returns immediately with a uuid. IRE will wake you up via --resume when the process finishes.',
    inputSchema: {
      type: 'object',
      properties: {
        name: { type: 'string', description: 'Short human-readable experiment name' },
        command: { type: 'string', description: 'Shell command to run (passed to sh -c)' },
        working_dir: { type: 'string', description: 'Working directory (defaults to workspace root)' },
        wake_prompt: { type: 'string', description: 'Prompt to send when the experiment finishes' },
      },
      required: ['name', 'command', 'wake_prompt'],
    },
  },
  {
    name: 'experiment.status',
    description: 'Get the status of an experiment by uuid.',
    inputSchema: {
      type: 'object',
      properties: {
        uuid: { type: 'string', description: 'Experiment uuid returned by experiment.start' },
      },
      required: ['uuid'],
    },
  },
  {
    name: 'experiment.list',
    description: 'List recent experiments.',
    inputSchema: {
      type: 'object',
      properties: {
        limit: { type: 'number', description: 'Max results (default 20)' },
      },
    },
  },
  {
    name: 'experiment.tail_logs',
    description: 'Return the tail of stdout and stderr logs for an experiment.',
    inputSchema: {
      type: 'object',
      properties: {
        uuid: { type: 'string', description: 'Experiment uuid' },
        kb: { type: 'number', description: 'Kilobytes to tail (default 64)' },
      },
      required: ['uuid'],
    },
  },
];

const server = new Server(
  { name: 'ire', version: '1.0.0' },
  { capabilities: { tools: {} } },
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({ tools: TOOLS }));

server.setRequestHandler(CallToolRequestSchema, async (req) => {
  const { name, arguments: args } = req.params;
  try {
    const result = await callBackend(name, args ?? {});
    const text = typeof result === 'string' ? result : JSON.stringify(result, null, 2);
    return { content: [{ type: 'text', text }] };
  } catch (err) {
    throw new McpError(ErrorCode.InternalError, String(err));
  }
});

const transport = new StdioServerTransport();
await server.connect(transport);
