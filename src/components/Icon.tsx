interface Props {
  name: string;
  className?: string;
}

const paths: Record<string, JSX.Element> = {
  add: (
    <>
      <path d="M12 5v14" />
      <path d="M5 12h14" />
    </>
  ),
  add_link: (
    <>
      <path d="M12 5v6" />
      <path d="M9 8h6" />
      <path d="M10 14H8a4 4 0 0 1 0-8h2" />
      <path d="M14 18h2a4 4 0 0 0 0-8h-2" />
      <path d="M8 12h8" />
    </>
  ),
  arrow_upward: (
    <>
      <path d="M12 19V5" />
      <path d="M6 11l6-6 6 6" />
    </>
  ),
  build: (
    <>
      <path d="M14.7 6.3a4 4 0 0 0-5 5L4 17l3 3 5.7-5.7a4 4 0 0 0 5-5l-3 3-3-3 3-3z" />
    </>
  ),
  chat: (
    <>
      <path d="M21 15a4 4 0 0 1-4 4H8l-5 3V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4z" />
    </>
  ),
  chevron_right: <path d="M9 18l6-6-6-6" />,
  close: (
    <>
      <path d="M6 6l12 12" />
      <path d="M18 6L6 18" />
    </>
  ),
  delete: (
    <>
      <path d="M3 6h18" />
      <path d="M8 6V4h8v2" />
      <path d="M19 6l-1 14H6L5 6" />
      <path d="M10 11v5" />
      <path d="M14 11v5" />
    </>
  ),
  description: (
    <>
      <path d="M14 3H6v18h12V7z" />
      <path d="M14 3v4h4" />
      <path d="M8 13h8" />
      <path d="M8 17h5" />
    </>
  ),
  developer_board: (
    <>
      <rect x="5" y="5" width="14" height="14" rx="2" />
      <path d="M9 9h6v6H9z" />
      <path d="M9 1v4M15 1v4M9 19v4M15 19v4M1 9h4M1 15h4M19 9h4M19 15h4" />
    </>
  ),
  edit_document: (
    <>
      <path d="M14 3H6v18h12v-8" />
      <path d="M14 3v4h4" />
      <path d="M13 17l6-6 2 2-6 6h-2z" />
    </>
  ),
  edit_note: (
    <>
      <path d="M4 7h10" />
      <path d="M4 12h8" />
      <path d="M4 17h6" />
      <path d="M14 18l5-5 2 2-5 5h-2z" />
    </>
  ),
  expand_more: <path d="M6 9l6 6 6-6" />,
  folder: (
    <>
      <path d="M3 6h7l2 2h9v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
    </>
  ),
  folder_open: (
    <>
      <path d="M3 8h7l2 2h9" />
      <path d="M3 8v10a2 2 0 0 0 2 2h13l3-10H8l-2 4" />
    </>
  ),
  lightbulb: (
    <>
      <path d="M9 18h6" />
      <path d="M10 22h4" />
      <path d="M8 14a6 6 0 1 1 8 0c-1 1-1 2-1 3H9c0-1 0-2-1-3z" />
    </>
  ),
  memory: (
    <>
      <rect x="6" y="6" width="12" height="12" rx="2" />
      <path d="M9 9h6v6H9z" />
      <path d="M4 9h2M4 15h2M18 9h2M18 15h2" />
    </>
  ),
  progress_activity: (
    <>
      <path d="M12 2v4" />
      <path d="M12 18v4" />
      <path d="M4.9 4.9l2.8 2.8" />
      <path d="M16.3 16.3l2.8 2.8" />
      <path d="M2 12h4" />
      <path d="M18 12h4" />
      <path d="M4.9 19.1l2.8-2.8" />
      <path d="M16.3 7.7l2.8-2.8" />
    </>
  ),
  refresh: (
    <>
      <path d="M21 12a9 9 0 0 1-15 6.7" />
      <path d="M3 12a9 9 0 0 1 15-6.7" />
      <path d="M18 2v4h-4" />
      <path d="M6 22v-4h4" />
    </>
  ),
  science: (
    <>
      <path d="M9 3h6" />
      <path d="M10 3v6l-5 9a2 2 0 0 0 1.7 3h10.6A2 2 0 0 0 19 18l-5-9V3" />
      <path d="M8 15h8" />
    </>
  ),
  settings: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a8 8 0 0 0 .1-2l2-1.5-2-3.5-2.4 1a8 8 0 0 0-1.7-1L15 5.5h-4L10.6 8a8 8 0 0 0-1.7 1l-2.4-1-2 3.5 2 1.5a8 8 0 0 0 .1 2l-2.1 1.5 2 3.5 2.4-1a8 8 0 0 0 1.7 1l.4 2.5h4l.4-2.5a8 8 0 0 0 1.7-1l2.4 1 2-3.5z" />
    </>
  ),
  storage: (
    <>
      <ellipse cx="12" cy="5" rx="7" ry="3" />
      <path d="M5 5v6c0 1.7 3.1 3 7 3s7-1.3 7-3V5" />
      <path d="M5 11v6c0 1.7 3.1 3 7 3s7-1.3 7-3v-6" />
    </>
  ),
  target: (
    <>
      <circle cx="12" cy="12" r="8" />
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2v3M12 19v3M2 12h3M19 12h3" />
    </>
  ),
};

export function Icon({ name, className }: Props) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {paths[name] ?? paths.description}
    </svg>
  );
}
