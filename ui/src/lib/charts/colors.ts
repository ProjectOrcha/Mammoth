// Categorical colours that mean the same thing in both themes.
//
// The semantic tokens in app.css are the right source for *state* — an "ok"
// green should differ between a dark and a light field. They are the wrong
// source for *category*: `--accent` and `--info` are gold and pale blue in the
// dark theme and two shades of navy in the light one, so a chart keyed off them
// loses its legend the moment somebody flips the toggle.
//
// These five are picked to stay distinct in hue and to keep usable contrast
// against both #08192a and #fefefe.

export const CATEGORY = {
  gold: '#c9a227',
  blue: '#3d8fd1',
  slate: '#7d93a8',
  green: '#3aa76d',
  rust: '#d2694a',
} as const;

/** What a fragment IS. State — rebuilding, corrupt, absent — is drawn with
 *  stroke and dash instead, so the two never compete for the same channel. */
export const FRAGMENT_COLOUR = {
  data: CATEGORY.gold,
  'local-parity': CATEGORY.blue,
  'global-parity': CATEGORY.slate,
  replica: CATEGORY.green,
} as const;

/** Where bytes are coming from, on the flow sankey. */
export const FLOW_COLOUR: Record<string, string> = {
  clients: CATEGORY.blue,
  repair: CATEGORY.gold,
  balancer: CATEGORY.green,
  shuffle: CATEGORY.slate,
};
