import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import type { TComicsTypes } from '../library.types';
import { USER_PREF } from './user-pref-cache.context';

export interface IComicsTypeCtx {
    type: IUltraCompStateStateful<TComicsTypes>;
    init: () => void;
    next: () => void;
}

export const COMICS_TYPE_CTX: IComicsTypeCtx = ultraCompState({
    type: 'detail' as TComicsTypes,

    init: (comp: IComicsTypeCtx) => {
        const comicType = USER_PREF.getPref('comicType');
        if (comicType) comp.type.set(comicType);
    },

    next: (comp: IComicsTypeCtx) => {
        const currType = comp.type.get();
        const nextType = currType === 'cover' ? 'detail' : 'cover';
        comp.type.set(nextType);
        USER_PREF.setPref({ comicType: nextType });
    },
});
