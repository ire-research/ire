import { useEffect, useRef, useState } from "react";
import type { AskAnswer, AskBlockState, AskQuestion, AskQuestionOption } from "../../types";
import { useChat } from "../../state/chat";
import { Icon } from "../Icon";

interface Props {
  ask: AskBlockState;
  onSubmit: (answers: AskAnswer[]) => void;
}

const STAGE_HEIGHT = 380;
const SLIDE_MS = 240;

export function AskQuestionCard({ ask, onSubmit }: Props) {
  const setAskAnswer = useChat((s) => s.setAskAnswer);
  const markAskSubmitted = useChat((s) => s.markAskSubmitted);

  const N = ask.questions.length;
  const reviewStep = N;

  const [step, setStep] = useState(0);
  const [outgoing, setOutgoing] = useState<{ index: number; direction: 1 | -1 } | null>(null);
  const [editing, setEditing] = useState<number | null>(null);

  // Drop the outgoing panel after the animation finishes.
  useEffect(() => {
    if (!outgoing) return;
    const t = setTimeout(() => setOutgoing(null), SLIDE_MS + 20);
    return () => clearTimeout(t);
  }, [outgoing]);

  const goTo = (next: number) => {
    if (next === step || next < 0 || next > reviewStep) return;
    setOutgoing({ index: step, direction: next > step ? 1 : -1 });
    setStep(next);
  };
  const goNext = () => goTo(step + 1);
  const goPrev = () => goTo(step - 1);

  const handleSubmit = () => {
    const final = ask.answers.map((a) => a ?? "") as AskAnswer[];
    markAskSubmitted(ask.tool_id);
    onSubmit(final);
  };

  if (ask.submitted) {
    return <AnsweredSummary ask={ask} />;
  }

  return (
    <div className="w-full rounded-lg border border-outline-variant bg-surface-container-low overflow-hidden">
      <CardHeader isReview={step === reviewStep} step={step} total={N} />

      <div className="relative overflow-hidden" style={{ height: STAGE_HEIGHT }}>
        {outgoing && (
          <div
            key={`out-${outgoing.index}`}
            className={`absolute inset-0 will-change-transform ${outgoing.direction > 0 ? "animate-slide-out-left" : "animate-slide-out-right"}`}
          >
            {outgoing.index === reviewStep ? (
              <ReviewPanel ask={ask} onEdit={setEditing} />
            ) : (
              <QuestionPanel ask={ask} index={outgoing.index} interactive={false} onAdvance={goNext} setAskAnswer={setAskAnswer} />
            )}
          </div>
        )}

        <div
          key={`in-${step}`}
          className={`absolute inset-0 will-change-transform ${outgoing ? (outgoing.direction > 0 ? "animate-slide-in-right" : "animate-slide-in-left") : ""}`}
        >
          {step === reviewStep ? (
            <ReviewPanel ask={ask} onEdit={setEditing} />
          ) : (
            <QuestionPanel ask={ask} index={step} interactive onAdvance={goNext} setAskAnswer={setAskAnswer} />
          )}
        </div>
      </div>

      <NavFooter
        step={step}
        total={N}
        canGoBack={step > 0}
        onPrev={goPrev}
        onNext={goNext}
        onReview={() => goTo(reviewStep)}
        onSubmit={handleSubmit}
      />

      {editing !== null && (
        <EditModal
          ask={ask}
          questionIndex={editing}
          onClose={() => setEditing(null)}
          setAskAnswer={setAskAnswer}
        />
      )}
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Header

function CardHeader({ isReview, step, total }: { isReview: boolean; step: number; total: number }) {
  return (
    <div className="px-4 py-2.5 border-b border-outline-variant flex items-center gap-2.5 text-[12px]">
      {isReview ? (
        <Icon name="check" className="w-[14px] h-[14px] text-on-surface" />
      ) : (
        <Icon name="help_outline" className="w-[14px] h-[14px] text-on-surface-variant" />
      )}
      <span className="text-on-surface font-medium tracking-[0.01em]">
        {isReview ? "Review your answers" : "Claude is asking for input"}
      </span>
      <span className="ml-auto font-mono text-[10.5px] text-on-surface-variant/70">
        {isReview ? `${total} question${total === 1 ? "" : "s"}` : `${step + 1} of ${total}`}
      </span>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Question panel

interface QuestionPanelProps {
  ask: AskBlockState;
  index: number;
  interactive: boolean;
  onAdvance: () => void;
  setAskAnswer: (toolId: string, index: number, answer: AskAnswer | undefined) => void;
}

function QuestionPanel({ ask, index, interactive, onAdvance, setAskAnswer }: QuestionPanelProps) {
  const q = ask.questions[index];
  const current = ask.answers[index];

  const onPick = (opt: ExtendedOption, checked: boolean, otherText: string) => {
    const next = computeAnswer(q, current, opt, checked, otherText);
    setAskAnswer(ask.tool_id, index, next);
    // Auto-advance on single-select non-Other picks.
    if (interactive && !q.multi_select && !opt.isOther && checked) {
      setTimeout(onAdvance, 220);
    }
  };

  return <QuestionBody q={q} current={current} onPick={onPick} />;
}

// ─────────────────────────────────────────────────────────────────────────────
// Question body (shared with modal)

type ExtendedOption = (AskQuestionOption & { isOther?: false }) | { label: "Other"; isOther: true };

function expandOptions(q: AskQuestion): ExtendedOption[] {
  return [...q.options, { label: "Other", isOther: true }];
}

function QuestionBody({
  q,
  current,
  onPick,
}: {
  q: AskQuestion;
  current: AskAnswer | undefined;
  onPick: (opt: ExtendedOption, checked: boolean, otherText: string) => void;
}) {
  const opts = expandOptions(q);
  const initialOther = extractOtherText(current);
  const [otherText, setOtherText] = useState(initialOther);

  const isChecked = (opt: ExtendedOption): boolean => {
    if (q.multi_select) {
      const set = Array.isArray(current) ? current : [];
      return opt.isOther ? set.some((v) => v.startsWith("Other: ")) : set.includes(opt.label);
    }
    if (opt.isOther) return typeof current === "string" && current.startsWith("Other: ");
    return current === opt.label;
  };

  return (
    <div className="px-4 py-5 space-y-3 overflow-y-auto" style={{ height: STAGE_HEIGHT }}>
      <div className="flex items-center gap-2">
        <span className="inline-flex items-center px-1.5 py-0.5 rounded border border-outline-variant text-on-surface-variant text-[10.5px] font-mono tracking-[0.02em]">
          {q.header}
        </span>
      </div>
      <p className="text-on-surface text-[14px] leading-snug">{q.question}</p>

      <div className="space-y-1.5 pt-1">
        {opts.map((opt, i) => (
          <label
            key={i}
            className="ask-opt flex items-start gap-3 p-2.5 rounded border border-transparent hover:bg-surface-container cursor-pointer transition-colors"
          >
            <input
              type={q.multi_select ? "checkbox" : "radio"}
              name={`q-${q.header}`}
              checked={isChecked(opt)}
              onChange={(e) => onPick(opt, e.currentTarget.checked, otherText)}
            />
            <div className="min-w-0 flex-1">
              <div className="text-on-surface text-[13.5px] leading-tight">{opt.label}</div>
              {!opt.isOther && opt.description && (
                <div className="text-on-surface-variant text-[12px] leading-snug mt-0.5">{opt.description}</div>
              )}
              {opt.isOther && (
                <input
                  type="text"
                  placeholder="describe…"
                  value={otherText}
                  onChange={(e) => {
                    setOtherText(e.target.value);
                    onPick(opt, true, e.target.value);
                  }}
                  onFocus={() => onPick(opt, true, otherText)}
                  className="mt-1.5 w-full bg-surface-container-lowest border border-outline-variant rounded px-2.5 py-1.5 text-[12.5px] text-on-surface placeholder:text-on-surface-variant/50 focus:outline-none focus:border-primary/60"
                />
              )}
            </div>
          </label>
        ))}
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Review panel

function ReviewPanel({ ask, onEdit }: { ask: AskBlockState; onEdit: (i: number) => void }) {
  return (
    <div className="px-2 py-2 overflow-y-auto" style={{ height: STAGE_HEIGHT }}>
      <div className="px-2 pt-1 pb-2 text-on-surface-variant text-[12.5px]">
        Click any row to revise its answer.
      </div>
      {ask.questions.map((q, i) => {
        const summary = formatAnswer(ask.answers[i]);
        return (
          <button
            key={i}
            type="button"
            onClick={() => onEdit(i)}
            className="w-full text-left px-3 py-3 rounded border border-transparent hover:border-outline-variant hover:bg-surface-container transition-colors flex items-start gap-3"
          >
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2 mb-1">
                <span className="inline-flex items-center px-1.5 py-0.5 rounded border border-outline-variant text-on-surface-variant text-[10.5px] font-mono tracking-[0.02em]">
                  {q.header}
                </span>
              </div>
              <div className="text-on-surface-variant text-[12px] leading-snug mb-1">{q.question}</div>
              <div className="text-on-surface text-[13px] leading-snug">
                {summary ? `→ ${summary}` : <span className="text-on-surface-variant/60 italic">no answer</span>}
              </div>
            </div>
            <Icon name="edit_document" className="w-[14px] h-[14px] text-on-surface-variant/60 mt-1" />
          </button>
        );
      })}
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Nav footer

interface NavFooterProps {
  step: number;
  total: number;
  canGoBack: boolean;
  onPrev: () => void;
  onNext: () => void;
  onReview: () => void;
  onSubmit: () => void;
}

function NavFooter({ step, total, canGoBack, onPrev, onNext, onReview, onSubmit }: NavFooterProps) {
  const isReview = step === total;
  const isLastQuestion = step === total - 1;

  return (
    <div className="px-3 py-2 border-t border-outline-variant bg-surface-container-low flex items-center">
      <NavArrow direction="prev" disabled={!canGoBack} onClick={onPrev} />

      <div className="flex items-center gap-1.5 mx-auto">
        {Array.from({ length: total + 1 }).map((_, i) => (
          <span
            key={i}
            className={`ask-pdot ${i < step ? "done" : ""} ${i === step ? "current" : ""}`}
          />
        ))}
      </div>

      {isReview ? (
        <button
          type="button"
          onClick={onSubmit}
          className="px-3 py-1.5 text-[12px] rounded bg-accent text-accent-fg font-medium hover:opacity-90 transition-opacity"
        >
          Submit answers
        </button>
      ) : isLastQuestion ? (
        <button
          type="button"
          onClick={onReview}
          className="px-3 py-1.5 text-[12px] rounded bg-surface-container border border-outline-variant text-on-surface hover:bg-surface-container-high transition-colors flex items-center gap-1.5"
        >
          Review
          <Icon name="chevron_right" className="w-[12px] h-[12px]" />
        </button>
      ) : (
        <NavArrow direction="next" onClick={onNext} />
      )}
    </div>
  );
}

function NavArrow({ direction, disabled, onClick }: { direction: "prev" | "next"; disabled?: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={direction === "prev" ? "Previous" : "Next"}
      className="app-icon-button w-7 h-7 disabled:opacity-30 disabled:cursor-not-allowed disabled:hover:bg-transparent disabled:hover:text-on-surface-variant"
    >
      <Icon name={direction === "prev" ? "chevron_left" : "chevron_right"} className="w-[14px] h-[14px]" />
    </button>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Edit modal

function EditModal({
  ask,
  questionIndex,
  onClose,
  setAskAnswer,
}: {
  ask: AskBlockState;
  questionIndex: number;
  onClose: () => void;
  setAskAnswer: (toolId: string, index: number, answer: AskAnswer | undefined) => void;
}) {
  const q = ask.questions[questionIndex];
  const backdropRef = useRef<HTMLDivElement>(null);

  // Close on Escape.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      ref={backdropRef}
      className="fixed inset-0 z-50 flex items-center justify-center backdrop-blur-sm bg-black/55 animate-backdrop-in"
      onClick={(e) => {
        if (e.target === backdropRef.current) onClose();
      }}
    >
      <div className="animate-modal-in w-[560px] max-w-[90vw] rounded-lg border border-outline-variant bg-surface-container-low overflow-hidden shadow-2xl">
        <div className="px-4 py-2.5 border-b border-outline-variant flex items-center gap-2.5 text-[12px]">
          <Icon name="edit_document" className="w-[14px] h-[14px] text-on-surface-variant" />
          <span className="text-on-surface font-medium tracking-[0.01em]">Edit answer</span>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="app-icon-button ml-auto w-7 h-7"
          >
            <Icon name="close" className="w-[14px] h-[14px]" />
          </button>
        </div>

        <QuestionBody
          q={q}
          current={ask.answers[questionIndex]}
          onPick={(opt, checked, otherText) => {
            const next = computeAnswer(q, ask.answers[questionIndex], opt, checked, otherText);
            setAskAnswer(ask.tool_id, questionIndex, next);
          }}
        />

        <div className="px-3 py-2 border-t border-outline-variant bg-surface-container-low flex items-center justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="px-3 py-1.5 text-[12px] rounded bg-accent text-accent-fg font-medium hover:opacity-90 transition-opacity"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Answered (locked) summary

function AnsweredSummary({ ask }: { ask: AskBlockState }) {
  return (
    <div className="w-full rounded-lg border border-outline-variant bg-surface-container-low overflow-hidden opacity-90">
      <div className="px-4 py-2.5 border-b border-outline-variant flex items-center gap-2.5 text-[12px]">
        <Icon name="check" className="w-[14px] h-[14px] text-ok" />
        <span className="text-on-surface font-medium tracking-[0.01em]">Answered</span>
        <span className="ml-auto font-mono text-[10.5px] text-on-surface-variant/70">
          {ask.questions.length} question{ask.questions.length === 1 ? "" : "s"}
        </span>
      </div>
      <div className="divide-y divide-outline-variant/60">
        {ask.questions.map((q, i) => (
          <div key={i} className="px-4 py-3 space-y-1">
            <span className="inline-flex items-center px-1.5 py-0.5 rounded border border-outline-variant text-on-surface-variant text-[10.5px] font-mono tracking-[0.02em]">
              {q.header}
            </span>
            <div className="text-on-surface text-[13px] leading-snug">→ {formatAnswer(ask.answers[i]) || "—"}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers

function formatAnswer(a: AskAnswer | undefined): string {
  if (!a) return "";
  if (Array.isArray(a)) return a.length ? a.join(", ") : "";
  return a;
}

function extractOtherText(a: AskAnswer | undefined): string {
  if (!a) return "";
  if (Array.isArray(a)) {
    const o = a.find((v) => v.startsWith("Other: "));
    return o ? o.replace(/^Other: /, "") : "";
  }
  if (typeof a === "string" && a.startsWith("Other: ")) return a.replace(/^Other: /, "");
  return "";
}

function computeAnswer(
  q: AskQuestion,
  current: AskAnswer | undefined,
  opt: ExtendedOption,
  checked: boolean,
  otherText: string,
): AskAnswer | undefined {
  if (q.multi_select) {
    const list = Array.isArray(current) ? [...current] : [];
    if (opt.isOther) {
      const filtered = list.filter((v) => !v.startsWith("Other: "));
      if (checked && otherText) filtered.push("Other: " + otherText);
      return filtered;
    }
    const set = new Set(list);
    if (checked) set.add(opt.label);
    else set.delete(opt.label);
    return [...set];
  }
  if (opt.isOther) {
    if (!checked) return undefined;
    return otherText ? "Other: " + otherText : "Other: ";
  }
  return checked ? opt.label : undefined;
}
