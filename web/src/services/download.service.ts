export function downloadComic(uid: string, name: string): void {
    const link = document.createElement('a');
    link.href = `/api/stream/${encodeURIComponent(uid)}`;
    link.download = name;
    document.body.appendChild(link);
    link.click();
    link.remove();
}
