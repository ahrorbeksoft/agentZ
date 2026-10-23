// The shimmer moves from Thinking to the Working line: the thought's row without it, the
// Working line with it, and the Working line while the agent waits for you.

TOPICS.push({
  id: 'thought', section: 'The Working line', title: 'A thought without the shimmer', size: 'wide', rec: 'A',
  now: 'A thought is a row among the tool calls: a light bulb and “Thinking”, with a band of the text color sweeping across the row’s gray every 2.2 seconds while the agent thinks, then “Thought”. A click opens the text. Settings › Threads › Show thinking (off by default) opens every thought. In every option Show thinking goes, thoughts start closed as they do today with it off, and a click still opens one.',
  nowImg: 'img/now-thinking.png',
  options: [
    { key: 'A', name: 'Thinking, held still', from: 'Zed (its thinking block’s still “Thinking”)',
      desc: '“Thinking” in the row’s gray while the agent thinks, then “Thought”, as today without the band.',
      good: 'As Zed shows it; the Working line under it says the agent is busy.', cost: 'Nothing on the row says it’s still going.',
      mock: () => ch2Thread({ convo: ch2Convo({ thinking: ch2Thought('Thinking'), working: ch2Working(`<span>Working for 1m 12s</span>${CH2_SHELL}`) }) }) },
    { key: 'B', name: 'Thinking…', from: 'new',
      desc: 'Like A, with an ellipsis while it thinks (“Thinking…”), then “Thought”.',
      good: 'Says it’s under way without moving.', cost: 'A small difference to notice.',
      mock: () => ch2Thread({ convo: ch2Convo({ thinking: ch2Thought('Thinking…'), working: ch2Working(`<span>Working for 1m 12s</span>${CH2_SHELL}`) }) }) },
    { key: 'C', name: 'A spinner for the bulb', from: 'agentZ’s Working line',
      desc: 'While it thinks, the Working line’s dots spin where the bulb is; “Thought” gets the bulb back.',
      good: 'Says it’s still going, without the band.', cost: 'Two spinners on screen at once.',
      mock: () => ch2Thread({ convo: ch2Convo({ thinking: ch2Thought('Thinking', { icon: CH2_DOTS }), working: ch2Working(`<span>Working for 1m 12s</span>${CH2_SHELL}`) }) }) },
  ],
});

const ch2WorkingThread = (line) => ch2Thread({ convo: ch2Convo({ thinking: ch2Thought('Thinking'), working: line }) });
TOPICS.push({
  id: 'working', section: 'The Working line', title: 'The shimmer on the Working line', size: 'wide', rec: 'A',
  now: 'The line the backlog calls “Running for” is the Working line at the end of a turn: the turn’s dots spinner, “Working for 1m 12s” in small muted text, then what goes on with it (“· 2 subthreads running”, “· 1 shell”, which opens the shells). Only the spinner moves. The mocks use A of the topic before for the thought, and are animated as the app would draw them.',
  nowImg: 'img/now-thinking.png',
  options: [
    { key: 'A', name: '“Working for 1m 12s” shimmers', from: 'agentZ’s Thinking shimmer (t3code’s live-tool-shine)',
      desc: 'Thinking’s band sweeps across “Working for 1m 12s” every 2.2 seconds. The spinner stays; the subthreads and shells after it hold still.',
      good: 'The shimmer moves as asked, and the parts you click stay easy to read.', cost: 'The spinner and the band both move.',
      mock: () => ch2WorkingThread(ch2Working(`${ch2Shim('Working for 1m 12s')}${CH2_SHELL}`)) },
    { key: 'B', name: 'Only “Working” shimmers', from: 'new',
      desc: 'Like A, but the band sweeps across “Working” alone; the time holds still.',
      good: 'The counting time stays easy to read.', cost: 'A short word for the band to cross.',
      mock: () => ch2WorkingThread(ch2Working(`<span>${ch2Shim('Working')} for 1m 12s</span>${CH2_SHELL}`)) },
    { key: 'C', name: 'The whole line', from: 'new',
      desc: 'Like A, but the band crosses everything after the spinner, the subthreads and shells too.',
      good: 'One sweep for the whole line.', cost: '“· 1 shell” is a button; a moving button is harder to aim for.',
      mock: () => ch2WorkingThread(ch2Working(ch2Shim(`Working for 1m 12s <span style="color:inherit">· 2 subthreads running · 1 shell</span>`))) },
    { key: 'D', name: 'The shimmer instead of the spinner', from: 't3code’s Working row',
      desc: 'Like A, without the dots: the band alone says it’s working, as t3code’s Working row has no spinner.',
      good: 'One thing moving.', cost: 'While it waits for you, the sand timer goes too (see the next topic).',
      mock: () => ch2WorkingThread(ch2Working(`${ch2Shim('Working for 1m 12s')}${CH2_SHELL}`, { spinner: '' })) },
  ],
});

const ch2WaitingThread = (line) => ch2Thread({ convo: ch2Convo({ between: ch2Permission(), working: line }), h: 480 });
TOPICS.push({
  id: 'waiting', section: 'The Working line', title: 'While the agent waits for you', size: 'wide', rec: 'A',
  now: 'While a permission request waits for you, the Working line’s spinner is a sand timer and the line starts with “Awaiting Confirmation”, before “Working for 1m 12s”. The mocks use A of the topic before.',
  nowImg: 'img/now-thinking.png',
  options: [
    { key: 'A', name: 'It holds still', from: 't3code (it shines only while the work goes on)',
      desc: 'The band stops while the agent waits for you, and comes back once you answer.',
      good: 'Nothing moves while nothing happens; the request is what to look at.', cost: 'The line changes when you answer.',
      mock: () => ch2WaitingThread(ch2Working('<span>Awaiting Confirmation</span><span>Working for 1m 12s</span>', { spinner: CH2_SAND })) },
    { key: 'B', name: 'It keeps shimmering', from: 'new',
      desc: '“Working for 1m 12s” shimmers as before; “Awaiting Confirmation” holds still before it.',
      good: 'The line looks the same through the turn.', cost: 'Says it’s working while it waits for you.',
      mock: () => ch2WaitingThread(ch2Working(`<span>Awaiting Confirmation</span>${ch2Shim('Working for 1m 12s')}`, { spinner: CH2_SAND })) },
    { key: 'C', name: '“Awaiting Confirmation” shimmers', from: 'new',
      desc: 'The band moves to “Awaiting Confirmation” while it waits; the time holds still.',
      good: 'Draws the eye to what needs you.', cost: 'The same effect means working and waiting.',
      mock: () => ch2WaitingThread(ch2Working(`${ch2Shim('Awaiting Confirmation')}<span>Working for 1m 12s</span>`, { spinner: CH2_SAND })) },
  ],
});
