import { useEffect, useRef, useState } from "react";
import type { AskAnswer, AskBlockState, AssistantMessage, ChatMessage } from "../../types";
import { AskQuestionCard } from "./AskQuestionCard";
import { ExperimentCard } from "./ExperimentCard";
import { MessageMarkdown } from "./MessageMarkdown";
import { ToolCard } from "./ToolCard";

interface MessageListProps {
  messages: ChatMessage[];
  onAskSubmit?: (ask: AskBlockState, answers: AskAnswer[]) => void;
}

export function MessageList({ messages, onAskSubmit }: MessageListProps) {
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
          <AssistantBubble key={m.id} msg={m as AssistantMessage} onAskSubmit={onAskSubmit} />
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

function AssistantBubble({ msg, onAskSubmit }: { msg: AssistantMessage; onAskSubmit?: (ask: AskBlockState, answers: AskAnswer[]) => void }) {
  // Timer: starts at mount (when the assistant turn begins), freezes when streaming stops.
  const startRef = useRef(Date.now());
  const [elapsed, setElapsed] = useState(msg.runtime ?? 0);

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
          return block.tool.kind === "experiment_start" ? (
            <ExperimentCard key={block.id} tool={block.tool} />
          ) : (
            <ToolCard key={block.id} tool={block.tool} />
          );
        }

        if (block.kind === "ask") {
          return (
            <AskQuestionCard
              key={block.id}
              ask={block.ask}
              onSubmit={(answers) => onAskSubmit?.(block.ask, answers)}
            />
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
          block.tool.status,
          block.tool.output?.full?.length ?? 0,
          block.tool.logLines?.length ?? 0,
        ].join(":");
      }
      if (block.kind === "ask") {
        return `ask:${block.ask.tool_id}:${block.ask.submitted}`;
      }
      return `${block.kind}:${block.text.length}`;
    })
    .join("|");
}
