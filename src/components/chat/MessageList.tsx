import { useEffect, useRef, useState } from "react";
import type { AssistantMessage, ChatMessage, ToolCallState } from "../../types";
import { ExperimentCard } from "./ExperimentCard";

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
    return (
      <div className="messages messages--empty">
        <p>Start a conversation. Brainstorm ideas or kick off an experiment.</p>
      </div>
    );
  }

  return (
    <div className="messages">
      {messages.map((m) =>
        m.role === "user" ? (
          <div key={m.id} className="message message--user">
            <div className="message__role">You</div>
            <div className="message__text">{m.text}</div>
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
  const [thinkingOpen, setThinkingOpen] = useState(false);
  const thinkingRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (msg.isStreaming && thinkingOpen && thinkingRef.current) {
      thinkingRef.current.scrollTop = thinkingRef.current.scrollHeight;
    }
  }, [msg.thinking, msg.isStreaming, thinkingOpen]);

  return (
    <div className="message message--assistant">
      <div className="message__role">Claude</div>

      {msg.thinking && (
        <div className="thinking-block">
          <button
            className="thinking-block__toggle"
            onClick={() => setThinkingOpen((v) => !v)}
          >
            {thinkingOpen ? "▾" : "▸"} Thinking
          </button>
          {thinkingOpen && (
            <div ref={thinkingRef} className="thinking-block__content">
              {msg.thinking}
            </div>
          )}
        </div>
      )}

      {msg.tools && msg.tools.length > 0 && (
        <div className="message__tools">
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
        <div className="message__text message__error">{msg.error}</div>
      ) : msg.text ? (
        <div className="message__text">{msg.text}</div>
      ) : msg.isStreaming ? (
        <div className="message__text">
          <span className="typing-dot" />
          <span className="typing-dot" />
          <span className="typing-dot" />
        </div>
      ) : null}
    </div>
  );
}

function ToolCard({ tool }: { tool: ToolCallState }) {
  const [expanded, setExpanded] = useState(false);
  const canExpand = tool.isDone && !!tool.output_full;

  return (
    <div className="tool-card-wrapper">
      <div
        className={`tool-card${canExpand ? " tool-card--clickable" : ""}`}
        onClick={() => canExpand && setExpanded((v) => !v)}
      >
        <span className="tool-card__name">[{tool.tool_name}]</span>
        {tool.input_preview && (
          <span className="tool-card__input"> {tool.input_preview}</span>
        )}
        {tool.isDone && tool.output_preview && (
          <>
            <span className="tool-card__sep"> ▸ </span>
            <span className="tool-card__preview">{tool.output_preview}</span>
          </>
        )}
        {!tool.isDone && <span className="tool-card__running"> …</span>}
      </div>
      {expanded && tool.output_full && (
        <pre className="tool-card__expanded">{tool.output_full}</pre>
      )}
    </div>
  );
}
