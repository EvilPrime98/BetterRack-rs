// Regenerates fuse_golden.json from the real Fuse.js + V8 sort used by better-wiki.
//   node gen_fuse_golden.mjs <path to BetterRack>/node_modules/fuse.js/dist/fuse.mjs
import { writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
const { default: Fuse } = await import(pathToFileURL(resolve(process.argv[2])).href);

let seed = 12345;
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32);
const pick = (a) => a[Math.floor(rnd() * a.length)];

const words = ['Batman', 'Superman', 'Detective', 'Comics', 'Vol', 'Annual', 'Special', 'The', 'Amazing', 'Spider-Man',
  'Immortal', 'Hulk', 'Action', 'Flash', 'Green', 'Lantern', 'Wonder', 'Woman', 'Justice', 'League', 'of', 'America',
  'Teen', 'Titans', 'Nightwing', 'Robin', 'Joker', 'Year', 'One', 'Dark', 'Knight', 'Returns', 'Éclair', 'Ørsted', 'Straße', 'Saga', 'Paper', 'Girls'];
const title = () => {
  const n = 1 + Math.floor(rnd() * 5);
  let t = Array.from({ length: n }, () => pick(words)).join(' ');
  if (rnd() < 0.7) t += ' ' + Math.floor(rnd() * 900 + 1);
  if (rnd() < 0.3) t += ` (${1990 + Math.floor(rnd() * 35)})`;
  return t;
};
const mutate = (t) => {
  let s = t.replace(/\(.+\)/, '').trim();
  const r = rnd();
  if (r < 0.2) s = s.toLowerCase();
  else if (r < 0.4 && s.length > 4) { const i = Math.floor(rnd() * s.length); s = s.slice(0, i) + s.slice(i + 1); }
  else if (r < 0.5) s = s + ' ' + pick(words) + ' ' + pick(words) + ' ' + pick(words) + ' ' + pick(words);
  return s;
};

const fuseCases = [];
const add = (titles, query) => {
  const docs = titles.map((t) => ({ page: { title: t } }));
  const fuse = new Fuse(docs, { keys: ['page.title'], threshold: 0.4, includeScore: true });
  const res = fuse.search(query).map((r) => ({ idx: r.refIndex, score: r.score ?? null }));
  fuseCases.push({ titles, query, results: res });
};
for (let c = 0; c < 400; c++) {
  const n = 1 + Math.floor(rnd() * 12);
  const titles = Array.from({ length: n }, title);
  add(titles, mutate(pick(titles)));
}
add(['Batman 1', 'Batman 2'], '');
add(['Batman 1', 'Batman 2'], '   ');
add(['A'], 'A');
add(['Batman: Year One', 'Batman Year One Deluxe Edition Collected', 'Batman Vol 1 404'], 'Batman Year One');
add(['x'.repeat(40) + ' 1', 'x'.repeat(39) + ' 1', 'y'.repeat(40)], 'x'.repeat(40) + ' 1');
add(['Ünïcode Tïtle 5', 'Unicode Title 5'], 'unicode title 5');

const sortCases = [];
for (let c = 0; c < 400; c++) {
  const n = 1 + Math.floor(rnd() * 30);
  const scores = Array.from({ length: n }, () => {
    const r = rnd();
    if (r < 0.3) return null; // NaN
    if (r < 0.5) return Math.floor(rnd() * 3) * 10; // many ties
    return rnd() * 200;
  });
  const items = scores.map((s, i) => ({ i, score: s === null ? NaN : s }));
  items.sort((a, b) => b.score - a.score);
  sortCases.push({ scores, order: items.map((x) => x.i) });
}
writeFileSync(new URL('./fuse_golden.json', import.meta.url), JSON.stringify({ fuseCases, sortCases }));
console.log(fuseCases.length, sortCases.length);
