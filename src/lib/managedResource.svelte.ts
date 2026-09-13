/// Ticket 190: disclosure-open for the managed resource Details.
///
/// Module-level like `managedInstall` so open/closed survives tab switches
/// (Settings unmounts on navigation); metrics never live here — the page
/// keeps usage/error locally and refetches on mount, so numbers are always
/// fresh and closed costs nothing.

export const managedResourceDisclosure = $state({
  open: false,
});

export function setManagedResourceOpen(open: boolean): void {
  managedResourceDisclosure.open = open;
}
