import { useEffect, useRef, useState } from "react";
import type { AssistantMessage, ChatMessage, ToolCallState } from "../../types";
import { ExperimentCard } from "./ExperimentCard";
import { MessageMarkdown } from "./MessageMarkdown";

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

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length]);

  if (messages.length === 0) {
    return <div className="flex-1" />;
  }

  return (
    <div className="space-y-6">
      {messages.map((m) =>
        m.role === "user" ? (
          <div key={m.id} className="flex justify-end">
            <div className="bg-surface-container text-on-surface px-4 py-3 rounded border border-outline-variant max-w-[560px] text-[14px] leading-relaxed">
              <MessageMarkdown content={m.text} />
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

function AssistantBubble({ msg }: { msg: AssistantMessage }) {
  const thinkingRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (msg.isStreaming && thinkingRef.current) {
      thinkingRef.current.scrollTop = thinkingRef.current.scrollHeight;
    }
  }, [msg.thinking, msg.isStreaming]);

  return (
    <div className="flex flex-col items-start max-w-[720px] space-y-4">
      {msg.thinking && (
        <div className="flex gap-3 text-on-surface-variant text-[13px] w-full">
          <div className="w-px bg-outline-variant shrink-0 my-1" />
          <div ref={thinkingRef} className="italic py-1 opacity-80 text-xs">
            {msg.thinking}
          </div>
        </div>
      )}

      {msg.tools && msg.tools.length > 0 && (
        <div className="w-full space-y-2">
          {msg.tools.map((tool) =>
            isExperimentStart(tool.tool_name) ? (
              <ExperimentCard key={tool.tool_id} tool={tool} />
            ) : (
              <ToolCard key={tool.tool_id} tool={tool} />
            )
          )}
        </div>
      )}

      {msg.error ? (
        <div className="text-[14px] text-error">{msg.error}</div>
      ) : msg.text ? (
        <div className="text-on-surface text-[14px] leading-relaxed">
          <MessageMarkdown content={msg.text} />
        </div>
      ) : msg.isStreaming ? (
        <div className="text-on-surface-variant animate-pulse text-[14px]">▌</div>
      ) : null}
    </div>
  );
}

function ToolCard({ tool }: { tool: ToolCallState }) {
  const [expanded, setExpanded] = useState(false);
  const canExpand = !!(tool.input_full || tool.output_full);

  if (tool.isDone) {
    return (
      <div className="w-full flex flex-col">
        <div
          className="w-full bg-surface-container-low border border-outline-variant rounded px-3 py-2 flex items-center gap-3 text-xs cursor-pointer hover:bg-surface-container transition-colors"
          onClick={() => canExpand && setExpanded((v) => !v)}
        >
          <span className="material-symbols-outlined text-ok text-[16px]">check_circle</span>
          <span className="font-mono text-on-surface-variant flex-1">{tool.tool_name}</span>
        </div>
        {expanded && (
          <div className="p-3 bg-surface-container-lowest font-mono text-[11px] text-on-surface-variant overflow-x-auto h-32 leading-relaxed">
            {tool.output_full || tool.input_full}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="w-full bg-surface-container border border-warn/40 rounded flex flex-col overflow-hidden">
      <div
        className="bg-surface-container-high px-3 py-2 flex items-center gap-3 text-xs border-b border-warn/20 cursor-pointer"
        onClick={() => canExpand && setExpanded((v) => !v)}
      >
        <span className="material-symbols-outlined text-warn text-[16px] animate-spin">progress_activity</span>
        <span className="font-mono text-warn flex-1">{tool.tool_name}</span>
      </div>
      {expanded && (
        <div className="p-3 bg-surface-container-lowest font-mono text-[11px] text-on-surface-variant overflow-x-auto h-32 leading-relaxed">
          {tool.output_full || tool.input_full}
        </div>
      )}
    </div>
  );
}
