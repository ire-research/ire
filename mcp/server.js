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
        path: { type: 'string', description: 'Path relative to wiki root (e.g. "notes.md", "status/pulse.md")' },
      },
      required: ['path'],
    },
  },
  {
    name: 'wiki.write',
    description: 'Atomically write a wiki file. Updates _index.md and appends to log.md. Auto-committed for status/** paths; uncommitted for notes.md, ideas.md, resources/**.',
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
    description: 'Append content to a wiki file. Used for append-only files like log.md.',
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
    description: 'Append an entry to long-term memory (status/long-term.md) under a named section. Auto-committed. Use for architectural decisions, pivots, and durable insights.',
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
    description: "Append to today's short-term memory (status/short-term/YYYY-MM-DD.md). Auto-committed. Use for daily operational notes, debugging steps, experiment details.",
    inputSchema: {
      type: 'object',
      properties: {
        content: { type: 'string', description: 'Content to append to today\'s notes' },
      },
      required: ['content'],
    },
  },
  {
    name: 'memory.record_failure',
    description: 'Record a structured failure entry in status/failures.md. Auto-committed. Always use this when an approach is abandoned or proven wrong.',
    inputSchema: {
      type: 'object',
      properties: {
        method: { type: 'string', description: 'Name of the method or approach that failed' },
        reason: { type: 'string', description: 'Why it failed' },
        context_ref: { type: 'string', description: 'Optional reference (wiki path, experiment id)' },
      },
      required: ['method', 'reason'],
    },
  },
  {
    name: 'pulse.update',
    description: 'Update the research pulse (status/pulse.md). Auto-committed. Patch any combination of question, blocker, and focus.',
    inputSchema: {
      type: 'object',
      properties: {
        question: { type: 'string', description: 'Current research question' },
        blocker: { type: 'string', description: 'Current blocker or "none"' },
        focus: { type: 'string', description: 'One-line focus banner' },
      },
    },
  },
  {
    name: 'experiment.start',
    description: 'Spawn a detached experiment subprocess. Returns immediately with a uuid. IRE will wake you up via --resume when the process finishes.',
    inputSchema: {
      type: 'object',
      properties: {
        name: { type: 'string', description: 'Short human-readable experiment name' },
        plan_md: { type: 'string', description: 'Full experiment plan in markdown' },
        command: { type: 'string', description: 'Shell command to run (passed to sh -c)' },
        working_dir: { type: 'string', description: 'Working directory (defaults to workspace root)' },
        wake_prompt: { type: 'string', description: 'Prompt to send when the experiment finishes' },
      },
      required: ['name', 'plan_md', 'command', 'wake_prompt'],
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
