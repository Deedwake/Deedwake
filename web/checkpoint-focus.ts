export function restoreCheckpointFocus(
  startedOnLoad: boolean,
  documentFocus: Pick<Document, 'activeElement' | 'body'>,
  load: HTMLButtonElement,
  destination: HTMLElement,
): void {
  if (startedOnLoad && (documentFocus.activeElement === load || documentFocus.activeElement === documentFocus.body)) {
    destination.focus();
  }
}
