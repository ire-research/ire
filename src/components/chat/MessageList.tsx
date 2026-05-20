import { useEffect, useRef, useState } from "react";
import type { AssistantMessage, ChatMessage, ToolCallState } from "../../types";
import { ExperimentCard } from "./ExperimentCard";
import { MessageMarkdown } from "./MessageMarkdown";
import { Icon } from "../Icon";

// CC may prefix MCP tool names with the server name (e.g. "ire__experiment.start"
// or "mcp__ire__experiment__start"). Strip any prefix to get the bare tool name.
function bareToolName(name: string): string {
  // Split on __ and take everything after the last server-name segment
  const parts = name.split("__");
  if (parts.length === 1) return name;
  // The bare name is the last part; dots may have been converted to underscores
  // by some CC versions, so normalise underscores → dots for the comparison only.
  return parts[parts.length - 1].replace(/_/g, ".");
}

function isExperimentStart(toolName: string): boolean {
  return bareToolName(toolName) === "experiment.start";
}

interface MessageListProps {
  messages: ChatMessage[];
}

export function MessageList({ messages }: MessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  const lastMsg = messages[messages.length - 1];
  const lastMsgKey = messageScrollKey(lastMsg);
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length, lastMsgKey]);

  if (messages.length === 0) {
    return <div className="flex-1" />;
  }

  return (
    <div className="space-y-6">
      {messages.map((m) =>
        m.role === "user" ? (
          <div key={m.id} className="flex justify-end">
            <div className="bg-surface-container text-on-surface px-4 py-3 rounded border border-outline-variant max-w-[560px] text-[14px] leading-relaxed whitespace-pre-wrap">
              {m.text}
            </div>
          </div>
        ) : (
          <AssistantBubble key={m.id} msg={m as AssistantMessage} />
        )
      )}
      <div ref={bottomRef} />
    </div>
  );
}

function formatElapsed(s: number): string {
  const mins   = Math.floor(s / 60);
  const secs   = Math.floor(s % 60);
  const tenths = Math.floor((s % 1) * 10);
  return `${mins}:${secs.toString().padStart(2, "0")}.${tenths}`;
}

function AssistantBubble({ msg }: { msg: AssistantMessage }) {
  // Timer: starts at mount (when the assistant turn begins), freezes when streaming stops.
  const startRef = useRef(Date.now());
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    if (!msg.isStreaming) return;
    let raf: number;
    const tick = () => {
      setElapsed((Date.now() - startRef.current) / 1000);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [msg.isStreaming]);

  // Show timer once streaming has started (elapsed > 0) or is currently running.
  const showTimer = msg.isStreaming || elapsed > 0;

  return (
    <div className="flex flex-col items-start max-w-[720px] space-y-4">
      {msg.blocks.map((block) => {
        if (block.kind === "thinking") {
          return <ThinkingBlock key={block.id} text={block.text} isStreaming={msg.isStreaming} />;
        }

        if (block.kind === "tool") {
          return isExperimentStart(block.tool.tool_name) ? (
            <ExperimentCard key={block.id} tool={block.tool} />
          ) : (
            <ToolCard key={block.id} tool={block.tool} />
          );
        }

        return (
          <div key={block.id} className="text-on-surface text-[14px] leading-relaxed">
            <MessageMarkdown content={block.text} />
          </div>
        );
      })}

      {msg.error && (
        <div className="text-[14px] text-error">{msg.error}</div>
      )}

      {/* Loading row: dots (while streaming) + timer (whole turn). Always last. */}
      {showTimer && !msg.error && (
        <div className="flex items-center gap-2.5">
          {msg.isStreaming && (
            <div className="flex items-center gap-[5px]">
              <div className="w-[5px] h-[5px] rounded-full bg-on-surface-variant animate-dot-bounce" style={{ animationDelay: "0s" }} />
              <div className="w-[5px] h-[5px] rounded-full bg-on-surface-variant animate-dot-bounce" style={{ animationDelay: "0.18s" }} />
              <div className="w-[5px] h-[5px] rounded-full bg-on-surface-variant animate-dot-bounce" style={{ animationDelay: "0.36s" }} />
            </div>
          )}
          <span className="font-mono text-[12px] text-on-surface-variant/60 min-w-[48px] tracking-[0.02em]">
            {formatElapsed(elapsed)}
          </span>
        </div>
      )}
    </div>
  );
}

function ThinkingBlock({ text, isStreaming }: { text: string; isStreaming: boolean }) {
  const [thinkingOpen, setThinkingOpen] = useState(false);
  const thinkingRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (isStreaming && thinkingOpen && thinkingRef.current) {
      thinkingRef.current.scrollTop = thinkingRef.current.scrollHeight;
    }
  }, [text, isStreaming, thinkingOpen]);

  return (
    <div className="flex gap-3 text-on-surface-variant text-[13px] w-full">
      <div className="w-px bg-outline-variant shrink-0 my-1" />
      <div className="flex-1 min-w-0">
        <button
          type="button"
          className="italic py-1 opacity-80 text-xs hover:text-on-surface transition-colors"
          onClick={() => setThinkingOpen((v) => !v)}
        >
          thinking...
        </button>
        {thinkingOpen && (
          <div
            ref={thinkingRef}
            className="mt-1 max-h-40 overflow-y-auto whitespace-pre-wrap font-mono text-[11px] leading-relaxed"
          >
            {text}
          </div>
        )}
      </div>
    </div>
  );
}

function messageScrollKey(message: ChatMessage | undefined): string {
  if (!message) return "";
  if (message.role === "user") return message.text;

  return message.blocks
    .map((block) => {
      if (block.kind === "tool") {
        return [
          block.tool.tool_id,
          block.tool.isDone,
          block.tool.output_full?.length ?? 0,
          block.tool.logLines?.length ?? 0,
        ].join(":");
      }
      return `${block.kind}:${block.text.length}`;
    })
    .join("|");
}

function ToolCard({ tool }: { tool: ToolCallState }) {
  const [expanded, setExpanded] = useState(false);
  const canExpand = !!(tool.input_full || tool.output_full);
  const input = formatToolInput(tool);

  return (
    <div className="w-full flex flex-col">
      <div
        className="w-full bg-surface-container-low border border-outline-variant rounded px-3 py-2 flex items-center gap-3 text-xs cursor-pointer hover:bg-surface-container transition-colors"
        onClick={() => canExpand && setExpanded((v) => !v)}
      >
        <Icon name="build" className="w-[16px] h-[16px] text-on-surface-variant" />
        <span className="font-mono text-on-surface-variant flex-1">{tool.tool_name}</span>
      </div>
      {expanded && (
        <div className="bg-surface-container-lowest border-x border-b border-outline-variant rounded-b overflow-hidden font-mono text-[11px] leading-relaxed">
          {input && <ToolIoField label="IN" content={input} />}
          {tool.output_full && <ToolIoField label="OUT" content={tool.output_full} />}
        </div>
      )}
    </div>
  );
}

function ToolIoField({ label, content }: { label: string; content: string }) {
  return (
    <div className="grid grid-cols-[42px_minmax(0,1fr)] border-t border-outline-variant first:border-t-0">
      <div className="px-3 py-2 text-on-surface-variant/60 uppercase">{label}</div>
      <pre className="px-3 py-2 text-on-surface-variant whitespace-pre-wrap break-words overflow-x-auto">{content}</pre>
    </div>
  );
}

function formatToolInput(tool: ToolCallState): string | null {
  if (!tool.input_full) return null;

  try {
    const parsed = JSON.parse(tool.input_full);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      if (typeof parsed.command === "string" && parsed.command.length > 0) {
        return parsed.command;
      }

      const values = Object.values(parsed).filter((value): value is string => typeof value === "string" && value.length > 0);
      if (values.length === 1) return values[0];
    }
  } catch {
    // Fall back to the raw input below.
  }

  return tool.input_full;
}
