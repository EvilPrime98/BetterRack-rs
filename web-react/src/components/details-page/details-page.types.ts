import type { useNavigate } from 'react-router-dom';
import type { WikiAppearanceEntry } from 'better-wiki';

export type CreditRow = { label: string; names: string[] };
export type AppearingGroup = { label: string; entries: WikiAppearanceEntry[] };
export type Navigate = ReturnType<typeof useNavigate>;
