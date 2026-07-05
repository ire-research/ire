import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faCircleInfo, iconClass } from "../icons";

interface Props {
  onAnswer: (enabled: boolean) => void;
}

export function AnalyticsConsentModal({ onAnswer }: Props) {
  return (
    <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center">
      <div className="w-[360px] bg-surface-container border border-outline-variant rounded-lg flex flex-col shadow-2xl">
        <div className="flex items-center gap-2 px-4 pt-3.5 pb-3 border-b border-outline-variant shrink-0">
          <FontAwesomeIcon icon={faCircleInfo} className={`${iconClass.lg} shrink-0 text-on-surface-variant`} />
          <span className="flex-1 text-[13px] font-medium text-on-surface">Help improve IRE</span>
        </div>

        <div className="px-4 pt-3.5 pb-4 flex flex-col gap-3">
          <p className="text-[12px] text-on-surface-variant">
            Sharing anonymous, minimal usage data (app launches, session length) helps us understand how IRE is used and make it better. No file paths, chat content, or personal data is ever sent. You can change this anytime.
          </p>
          <div className="flex items-center justify-end gap-2">
            <button
              onClick={() => onAnswer(false)}
              className="border border-outline-variant text-on-surface-variant px-3 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors"
            >
              No thanks
            </button>
            <button
              onClick={() => onAnswer(true)}
              className="border border-outline text-on-surface px-3 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors"
            >
              Yes, help improve IRE
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
