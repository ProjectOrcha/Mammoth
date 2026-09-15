export type Workspace = 'gateway' | 'demo';

/** An explicit choice is scoped to this tab; a server failure never changes it. */
export function chosenWorkspace(): Workspace | null {
  if (typeof window === 'undefined') return null;
  const query = new URLSearchParams(window.location.search).get('workspace');
  if (query === 'demo' || query === 'gateway') {
    try { sessionStorage.setItem('mammoth:workspace', query); } catch { /* URL still works */ }
    return query;
  }
  try {
    const saved = sessionStorage.getItem('mammoth:workspace');
    return saved === 'demo' || saved === 'gateway' ? saved : null;
  } catch { return null; }
}

export function switchWorkspace(workspace: Workspace) {
  const path = window.location.pathname.startsWith('/files/') ? '/files' : window.location.pathname;
  window.location.assign(`${path}?workspace=${workspace}`);
}
