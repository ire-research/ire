interface FocusBannerProps {
  focus: string;
}

export function FocusBanner({ focus }: FocusBannerProps) {
  return (
    <div className="focus-banner">
      <div className="focus-banner__label">FOCUS</div>
      <div className="focus-banner__text">{focus}</div>
    </div>
  );
}
