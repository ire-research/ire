import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface Props {
  title: string;
  content: string;
}

export function ResourcePreviewPane({ title, content }: Props) {
  return (
    <div className="absolute inset-0 overflow-y-auto px-4 md:px-8 lg:px-12 pt-6 pb-8">
      <p className="text-[11px] uppercase tracking-widest text-on-surface-variant mb-4">Resource · Wiki</p>
      <h2 className="text-base font-semibold text-on-surface mb-2">{title}</h2>
      <div className="text-[14px] text-on-surface leading-relaxed">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
      </div>
    </div>
  );
}
