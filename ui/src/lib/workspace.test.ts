import { afterEach, expect, it } from 'vitest';
import { chosenWorkspace } from './workspace';
afterEach(() => { sessionStorage.clear(); window.history.replaceState({}, '', '/'); });

it('keeps an explicit workspace choice through navigation and reloads', () => {
  window.history.replaceState({}, '', '/cluster?workspace=demo');
  expect(chosenWorkspace()).toBe('demo');
  window.history.replaceState({}, '', '/distribution');
  expect(chosenWorkspace()).toBe('demo');
  window.history.replaceState({}, '', '/distribution?workspace=gateway');
  expect(chosenWorkspace()).toBe('gateway');
});

it('ignores invalid workspace values', () => {
  window.history.replaceState({}, '', '/?workspace=invalid');
  expect(chosenWorkspace()).toBeNull();
});
