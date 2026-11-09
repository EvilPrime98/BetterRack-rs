export function normalizePath(dir: string): string {
    return dir.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
}

export function getTopLevelDirs(dirs: string[]): string[] {
    const normalized = dirs.map(normalizePath);
    return dirs.filter(
        (_, i) => !normalized.some((other, j) => j !== i && normalized[i].startsWith(`${other}/`)),
    );
}

export function visibleDirectories(
    dirs: string[],
    { query, showSubfolders }: { query: string; showSubfolders: boolean },
): string[] {
    const candidates = showSubfolders ? dirs : getTopLevelDirs(dirs);
    const needle = query.trim().toLowerCase();
    return needle ? candidates.filter((dir) => dir.toLowerCase().includes(needle)) : candidates;
}
