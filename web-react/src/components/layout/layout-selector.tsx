import { BRDropdown, type BRDropdownOption } from '@/components/br-dropdown/br-dropdown';
import { useComicsTypeStore } from '@/stores/comicsTypes.store';
import type { TComicsTypes } from '@/library.types';

const OPTIONS: BRDropdownOption<TComicsTypes>[] = [
    { value: 'cover', label: 'Cover' },
    { value: 'detail', label: 'Detail' },
];

export function LayoutSelector() {
    const type = useComicsTypeStore((s) => s.type);
    const set = useComicsTypeStore((s) => s.set);

    return (
        <BRDropdown<TComicsTypes>
            options={OPTIONS}
            value={type}
            onChange={set}
            alignRight
            fit
            aria-label="Layout"
        />
    );
}
