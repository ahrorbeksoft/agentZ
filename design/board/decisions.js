// A round's picks as Markdown, the spec agents build from. The board sends it with every
// save; from a shell, `node design/board/decisions.js <area>` writes it again.

function plainText(html) {
  return String(html ?? '')
    .replace(/<code>(.*?)<\/code>/g, '`$1`')
    .replace(/<[^>]+>/g, '')
    .replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&amp;/g, '&')
    .replace(/\s+/g, ' ')
    .trim();
}

function decisionsMarkdown(topics, state, { title, area }) {
  const lines = [
    `# ${title}: design decisions`,
    '',
    `Picked on the design board (\`design/${area}/\`, see \`design/README.md\`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from \`choices.json\`; don't edit by hand.`,
    '',
  ];
  topics.forEach((topic, index) => {
    const s = state.topics?.[topic.id] || {};
    const keys = topic.type === 'multi' ? s.picks || [] : s.pick ? [s.pick] : [];
    const picked = keys.map((key) => topic.options.find((option) => option.key === key)).filter(Boolean);
    const also = topic.type === 'multi' || !s.pick ? [] : (s.also || []).filter((key) => key !== s.pick).map((key) => topic.options.find((option) => option.key === key)).filter(Boolean);
    const heading = s.none ? 'None of these' : picked.length ? [...picked.map((o) => `${o.key}. ${o.name}`), ...also.map((o) => `also ${o.key}. ${o.name}`)].join(' + ') : 'Not decided yet';
    lines.push(`## ${index + 1}. ${topic.title}: ${heading}`, '');
    lines.push(`*${topic.section}${topic.type === 'multi' ? ' · pick any' : ''}*`, '');
    lines.push(`**Today:** ${plainText(topic.now)}`, '');
    for (const option of picked) {
      lines.push(`**${option.key}. ${option.name}**${option.from ? ` (from ${option.from})` : ''}: ${plainText(option.desc)}`, '');
    }
    for (const option of also) {
      lines.push(`**Also take from ${option.key}. ${option.name}**${option.from ? ` (from ${option.from})` : ''}: ${plainText(option.desc)}`, '');
    }
    for (const [key, comment] of Object.entries(s.comments || {})) {
      if (comment.trim()) lines.push(`**Comment on ${key}:** ${comment.trim()}`, '');
    }
    if ((s.note || '').trim()) lines.push(`**Note:** ${s.note.trim()}`, '');
  });
  return lines.join('\n');
}

if (typeof module !== 'undefined' && require.main === module) {
  const fs = require('fs');
  const path = require('path');
  const vm = require('vm');
  const area = process.argv[2];
  if (!area) {
    console.error('usage: node design/board/decisions.js <area>');
    process.exit(1);
  }
  const directory = path.join(__dirname, '..', area);
  const page = fs.readFileSync(path.join(directory, 'index.html'), 'utf8');
  const title = (page.match(/data-title="([^"]+)"/) || [])[1] || area;
  // The round's own scripts, in the order its page loads them.
  const topicFiles = [...page.matchAll(/<script src="([^"]+)"/g)].map((m) => m[1]).filter((src) => !src.startsWith('../'));
  const context = { console };
  vm.createContext(context);
  const source = [fs.readFileSync(path.join(__dirname, 'mock.js'), 'utf8'), 'const TOPICS = [];']
    .concat(topicFiles.map((file) => fs.readFileSync(path.join(directory, file), 'utf8')))
    .concat('this.TOPICS = TOPICS;')
    .join('\n');
  vm.runInContext(source, context);
  let state = {};
  try {
    state = JSON.parse(fs.readFileSync(path.join(directory, 'choices.json'), 'utf8'));
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  fs.writeFileSync(path.join(directory, 'decisions.md'), decisionsMarkdown(context.TOPICS, state, { title, area }));
  console.log(`wrote design/${area}/decisions.md`);
}
