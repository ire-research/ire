// Icon size tokens — change a value here to resize all icons at that level globally.
// Note: with FA Free, the thinnest available weight is "regular" (applied below).
// To get FA Pro Light/Thin, replace "@fortawesome/free-regular-svg-icons" with
// "@fortawesome/pro-light-svg-icons" or "@fortawesome/pro-thin-svg-icons".
import type { IconDefinition, IconName } from "@fortawesome/fontawesome-svg-core";
import { faClaude } from "@fortawesome/free-brands-svg-icons/faClaude";
import { faOpenai } from "@fortawesome/free-brands-svg-icons/faOpenai";
import { faCircleQuestion } from "@fortawesome/free-regular-svg-icons/faCircleQuestion";
import { faFileLines } from "@fortawesome/free-regular-svg-icons/faFileLines";
import { faFolder } from "@fortawesome/free-regular-svg-icons/faFolder";
import { faFolderOpen } from "@fortawesome/free-regular-svg-icons/faFolderOpen";
import { faLightbulb } from "@fortawesome/free-regular-svg-icons/faLightbulb";
import { faMessage } from "@fortawesome/free-regular-svg-icons/faMessage";
import { faPenToSquare } from "@fortawesome/free-regular-svg-icons/faPenToSquare";
import { faAnglesRight } from "@fortawesome/free-solid-svg-icons/faAnglesRight";
import { faArrowUp } from "@fortawesome/free-solid-svg-icons/faArrowUp";
import { faCheck } from "@fortawesome/free-solid-svg-icons/faCheck";
import { faChevronDown } from "@fortawesome/free-solid-svg-icons/faChevronDown";
import { faChevronLeft } from "@fortawesome/free-solid-svg-icons/faChevronLeft";
import { faChevronRight } from "@fortawesome/free-solid-svg-icons/faChevronRight";
import { faCircleExclamation } from "@fortawesome/free-solid-svg-icons/faCircleExclamation";
import { faCircleInfo } from "@fortawesome/free-solid-svg-icons/faCircleInfo";
import { faClockRotateLeft } from "@fortawesome/free-solid-svg-icons/faClockRotateLeft";
import { faCrosshairs } from "@fortawesome/free-solid-svg-icons/faCrosshairs";
import { faDatabase } from "@fortawesome/free-solid-svg-icons/faDatabase";
import { faFlask } from "@fortawesome/free-solid-svg-icons/faFlask";
import { faGamepad } from "@fortawesome/free-solid-svg-icons/faGamepad";
import { faGear } from "@fortawesome/free-solid-svg-icons/faGear";
import { faLink } from "@fortawesome/free-solid-svg-icons/faLink";
import { faMagnifyingGlass } from "@fortawesome/free-solid-svg-icons/faMagnifyingGlass";
import { faMicrochip } from "@fortawesome/free-solid-svg-icons/faMicrochip";
import { faPencil } from "@fortawesome/free-solid-svg-icons/faPencil";
import { faPlus } from "@fortawesome/free-solid-svg-icons/faPlus";
import { faSpinner } from "@fortawesome/free-solid-svg-icons/faSpinner";
import { faTerminal } from "@fortawesome/free-solid-svg-icons/faTerminal";
import { faTrash } from "@fortawesome/free-solid-svg-icons/faTrash";
import { faWrench } from "@fortawesome/free-solid-svg-icons/faWrench";
import { faXmark } from "@fortawesome/free-solid-svg-icons/faXmark";

export const iconClass = {
  xs: "size-[10px]",  // status bar
  sm: "size-[11px]",  // compact buttons, tab icons
  md: "size-[12px]",  // standard actions
  lg: "size-[13px]",  // section headers, navigation
} as const;

// Central icon registry. All FA icon imports flow through here.

export const faSidebarLeft: IconDefinition = {
  prefix: "fak",
  iconName: "sidebar-left" as IconName,
  icon: [
    512,
    512,
    [],
    "e001",
    "M64 96c-8.8 0-16 7.2-16 16v288c0 8.8 7.2 16 16 16h384c8.8 0 16-7.2 16-16V112c0-8.8-7.2-16-16-16H64zM0 112c0-35.3 28.7-64 64-64h384c35.3 0 64 28.7 64 64v288c0 35.3-28.7 64-64 64H64c-35.3 0-64-28.7-64-64V112zm96 48c0-8.8 7.2-16 16-16h96c8.8 0 16 7.2 16 16v192c0 8.8-7.2 16-16 16h-96c-8.8 0-16-7.2-16-16V160z",
  ],
};

export const faSidebarRight: IconDefinition = {
  prefix: "fak",
  iconName: "sidebar-right" as IconName,
  icon: [
    512,
    512,
    [],
    "e002",
    "M64 96c-8.8 0-16 7.2-16 16v288c0 8.8 7.2 16 16 16h384c8.8 0 16-7.2 16-16V112c0-8.8-7.2-16-16-16H64zM0 112c0-35.3 28.7-64 64-64h384c35.3 0 64 28.7 64 64v288c0 35.3-28.7 64-64 64H64c-35.3 0-64-28.7-64-64V112zm288 48c0-8.8 7.2-16 16-16h96c8.8 0 16 7.2 16 16v192c0 8.8-7.2 16-16 16h-96c-8.8 0-16-7.2-16-16V160z",
  ],
};

export {
  faAnglesRight,
  faArrowUp,
  faCheck,
  faChevronDown,
  faChevronLeft,
  faChevronRight,
  faCircleExclamation,
  faCircleInfo,
  faClockRotateLeft,
  faCrosshairs,
  faDatabase,
  faFlask,
  faGamepad,
  faGear,
  faLink,
  faMagnifyingGlass,
  faMicrochip,
  faPencil,
  faPlus,
  faSpinner,
  faTerminal,
  faTrash,
  faWrench,
  faXmark,
  faCircleQuestion,
  faClaude,
  faFileLines,
  faFolder,
  faFolderOpen,
  faLightbulb,
  faMessage,
  faOpenai,
  faPenToSquare,
};
