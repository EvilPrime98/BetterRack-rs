import { ultraCompState } from 'ultra-light-js';
import type { WikiComic } from 'better-wiki';
import type { TMetaSource } from '@/library.types';

export const COMIC_IDENT_CTX = ultraCompState({
    isVisible: false,
    itemUid: '',
    lastIdentified: null as { uid: string; comic: WikiComic; metaSource: TMetaSource } | null,
    lastUnidentified: null as { uid: string } | null,
});
